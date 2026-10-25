#!/bin/sh
# Build both demo-escrow flavors from source to target/sbf-programs/.
# Requires: cargo-build-sbf (anza-xyz/cargo-build-sbf 4.5.0) and the Solana platform-tools v1.58
# extracted at $TOKENSCOPE_PLATFORM_TOOLS (default: ~/.cache/solana/v1.58/platform-tools).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PT="${TOKENSCOPE_PLATFORM_TOOLS:-$HOME/.cache/solana/v1.58/platform-tools}"
OUT="$ROOT/target/sbf-programs"
[ -x "$PT/rust/bin/rustc" ] || { echo "platform-tools not found at $PT (see docs/toolchain.md)" >&2; exit 1; }
mkdir -p "$OUT"
unset RUSTFLAGS
for flavor in vulnerable corrected; do
  features=""
  [ "$flavor" = corrected ] && features="--features corrected"
  tmp=$(mktemp -d)
  (cd "$ROOT/programs/demo-escrow" && \
    PATH="$PT/rust/bin:$PATH" RUSTC="$PT/rust/bin/rustc" \
    cargo-build-sbf --no-rustup-override --skip-tools-install --tools-version v1.58 \
      --sbf-out-dir "$tmp" $features >&2)
  cp "$tmp/demo_escrow.so" "$OUT/demo_escrow_$flavor.so"
  rm -rf "$tmp"
  echo "built $OUT/demo_escrow_$flavor.so"
done
