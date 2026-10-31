#!/usr/bin/env python3
"""Verify that `tokenscope tui` restores the terminal on normal exit, on an error and on a panic.

usage: scripts/check-tui-terminal.py [--normal-binary PATH] [--fault-binary PATH]

The binary for the error/panic cases must be built with the test-only hook:
    cargo build -p tokenscope-cli --features fault-injection --target-dir target/fault
(default binary paths: target/debug/tokenscope and target/fault/debug/tokenscope).

SIGTERM and SIGHUP are handled (the event loop exits and restores the terminal); SIGKILL cannot be.
Each case runs the TUI on a pseudo-terminal, records the terminal attributes (termios) of the
slave side before and after, and checks the emitted escape sequences: the alternate screen must be
entered and left, the cursor shown again, and the termios flags identical to the original (raw
mode disabled). Exits non-zero on any failure.
"""
import argparse, os, pty, select, signal, subprocess, sys, termios, time

ENTER_ALT = b"\x1b[?1049h"
LEAVE_ALT = b"\x1b[?1049l"
SHOW_CURSOR = b"\x1b[?25h"


def run_case(binary, args, env_extra, keys, wait_exit=8.0, kill_signal=None):
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    env = dict(os.environ, TERM="xterm-256color", **env_extra)
    errf = os.path.join("/tmp", f"tokenscope-tui-err-{os.getpid()}-{time.time_ns()}")
    with open(errf, "wb") as err:
        proc = subprocess.Popen([binary, *args], stdin=slave, stdout=slave, stderr=err, env=env, close_fds=True)
    out = b""

    def pump(seconds):
        nonlocal out
        end = time.time() + seconds
        while time.time() < end:
            r, _, _ = select.select([master], [], [], 0.05)
            if r:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    return
                if not chunk:
                    return
                out += chunk
            if proc.poll() is not None and not r:
                return

    pump(2.0)  # let it draw a few frames (the fault, if any, fires on frame 3)
    if kill_signal is not None and proc.poll() is None:
        proc.send_signal(kill_signal)
    elif keys and proc.poll() is None:
        os.write(master, keys)
    deadline = time.time() + wait_exit
    while proc.poll() is None and time.time() < deadline:
        pump(0.2)
    pump(0.3)
    if proc.poll() is None:
        proc.kill()
        proc.wait()
    after = termios.tcgetattr(slave)
    err_text = open(errf, "rb").read().decode(errors="replace")
    os.unlink(errf)
    os.close(master)
    os.close(slave)
    return proc.returncode, out, before == after, err_text


def check(name, ok, detail=""):
    print(f"[{'ok' if ok else 'FAIL'}] {name}" + (f"  ({detail})" if detail and not ok else ""))
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--normal-binary", default="target/debug/tokenscope")
    ap.add_argument("--fault-binary", default="target/fault/debug/tokenscope")
    a = ap.parse_args()
    ok = True
    tui = ["tui", "--target", "demo-escrow-corrected", "--no-suite", "--count", "1"]

    code, out, restored, err = run_case(a.normal_binary, tui, {}, b"q")
    ok &= check("normal exit (q): exit code 0", code == 0, f"code={code} stderr={err[:200]}")
    ok &= check("normal exit: alternate screen entered then left", ENTER_ALT in out and LEAVE_ALT in out and out.rfind(LEAVE_ALT) > out.find(ENTER_ALT))
    ok &= check("normal exit: cursor shown again", SHOW_CURSOR in out)
    ok &= check("normal exit: termios identical to the original (raw mode off)", restored)

    code, out, restored, err = run_case(a.normal_binary, tui, {}, b"\x03")
    ok &= check("Ctrl-C key: exit code 0 and terminal restored", code == 0 and restored and LEAVE_ALT in out, f"code={code}")

    if os.path.exists(a.fault_binary):
        for fault, want_code, want_msg in (("error", 3, "injected error"), ("panic", 101, "injected panic")):
            code, out, restored, err = run_case(a.fault_binary, tui, {"TOKENSCOPE_TUI_FAULT": fault}, None)
            ok &= check(f"{fault}: exit code {want_code}", code == want_code, f"code={code}")
            ok &= check(f"{fault}: message on stderr", want_msg in err, err[:200])
            ok &= check(f"{fault}: alternate screen left, cursor shown", LEAVE_ALT in out and SHOW_CURSOR in out)
            ok &= check(f"{fault}: termios identical to the original", restored)
    else:
        ok &= check(f"fault binary present at {a.fault_binary}", False, "build it with --features fault-injection")

    for sig, name in ((signal.SIGTERM, "SIGTERM"), (signal.SIGHUP, "SIGHUP")):
        code, out, restored, err = run_case(a.normal_binary, tui, {}, None, kill_signal=sig)
        ok &= check(f"{name}: exits cleanly (code 0) and restores the terminal",
                    code == 0 and restored and LEAVE_ALT in out and SHOW_CURSOR in out,
                    f"code={code} restored={restored}")
    print("[info] SIGKILL cannot be handled by any process; a killed TUI leaves the terminal in raw mode (run `reset`).")
    sys.exit(0 if ok else 1)


main()
