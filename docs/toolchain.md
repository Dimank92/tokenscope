# Toolchain and installation (verified procedure)

Everything below was executed in the development sandbox (Linux x86_64) unless marked otherwise.
Prefer your platform's official installers when they are reachable; the commands below exist
because the sandbox's network allowlist blocked `static.rust-lang.org` and `release.anza.xyz`.

| Component | Version | Source |
|---|---|---|
| Rust (rustc, cargo, clippy, rustfmt) | 1.99.0 | `rustup` (`rust-toolchain.toml` pins it) or conda-forge `rust=1.99.0` |
| `cargo-build-sbf` | 4.5.0 | `cargo install cargo-build-sbf --version 4.5.0 --locked` (crates.io, owner `anza-team`, repo `anza-xyz/cargo-build-sbf`) |
| Solana platform-tools | v1.58 | `https://github.com/anza-xyz/platform-tools/releases/download/v1.58/platform-tools-linux-x86_64.tar.bz2` |
| RTK (development aid only) | v1.0.1 | official `install.sh` with `RTK_VERSION=v1.0.1` (verifies the archive SHA-256) |

## Platform-tools

```sh
mkdir -p ~/.cache/solana/v1.58/platform-tools && cd ~/.cache/solana/v1.58/platform-tools
curl -L -o /tmp/pt.tar.bz2 https://github.com/anza-xyz/platform-tools/releases/download/v1.58/platform-tools-linux-x86_64.tar.bz2
sha256sum /tmp/pt.tar.bz2   # expected 7b82eb420b5aebd1e5590db0ae25c523eca06724d4238ae02139690535636c37
tar xjf /tmp/pt.tar.bz2     # (the digest above equals the one GitHub publishes for the asset)
```

`scripts/build-programs.sh` calls `cargo-build-sbf --no-rustup-override --skip-tools-install
--tools-version v1.58` with the platform-tools Rust first on `PATH` and `RUSTFLAGS` unset. The
`--no-rustup-override` flag is required when `rustup` is not installed; override the location with
`TOKENSCOPE_PLATFORM_TOOLS`.

## Notes from setup

* `cargo install cargo-build-sbf` needs OpenSSL **3.x** development files (`openssl-sys` rejected
  OpenSSL 4.1). We used an isolated conda-forge env (`openssl>=3,<4`, `OPENSSL_DIR`), without
  touching host packages.
* Agave-line crates are large; a clean host build took about 3 minutes on one CPU and about 650 MB
  in `target/`. The bundled `.cargo/config.toml` silences the runtime's debug logging in tests.
* RTK is optional tooling and is not a TokenScope dependency. Hooks that rewrite shell commands
  were not installed in the sandbox; `rtk cargo ...` was invoked explicitly.
