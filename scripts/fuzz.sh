#!/bin/sh
# Build and run one fuzz target with libFuzzer coverage feedback on the STABLE compiler.
#
# usage: scripts/fuzz.sh <target> [seconds]     (default 30 s)
#
# The flags are the SanitizerCoverage flags `cargo fuzz` applies, minus the nightly-only
# AddressSanitizer. With a nightly toolchain you can instead run: cargo +nightly fuzz run <target>
# from the repository root. New inputs go to a scratch directory; the committed seed corpus in
# fuzz/corpus/<target>/ is only read. Crashes land in fuzz/artifacts/<target>/.
set -eu
TARGET=${1:?usage: scripts/fuzz.sh <target> [seconds]}
SECS=${2:-30}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${TOKENSCOPE_FUZZ_TARGET_DIR:-"${TMPDIR:-/tmp}/tokenscope-fuzz-target"}
SCRATCH=$(mktemp -d)
ART="$ROOT/fuzz/artifacts/$TARGET"
mkdir -p "$ART"
FLAGS="--cfg fuzzing -Cpasses=sancov-module -Cllvm-args=-sanitizer-coverage-level=4 \
-Cllvm-args=-sanitizer-coverage-inline-8bit-counters -Cllvm-args=-sanitizer-coverage-pc-table \
-Cllvm-args=-sanitizer-coverage-trace-compares -Cdebug-assertions -Ccodegen-units=1 -Cdebuginfo=0"
HOST=$(rustc -vV | sed -n 's/^host: //p')
# An explicit --target keeps the flags off build scripts and proc macros (as cargo-fuzz does).
(cd "$ROOT/fuzz" && RUSTFLAGS="$FLAGS" CARGO_TARGET_DIR="$OUT" \
  cargo build --release --target "$HOST" --bin "$TARGET" >&2)
"$OUT/$HOST/release/$TARGET" -max_total_time="$SECS" -max_len=65536 -print_final_stats=1 \
  -artifact_prefix="$ART/" "$SCRATCH" "$ROOT/fuzz/corpus/$TARGET"
rm -rf "$SCRATCH"
