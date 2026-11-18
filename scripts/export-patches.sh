#!/bin/sh
# Export the full ordered patch series and regenerate PATCH_MANIFEST.md / PATCHES_README.md
# in the delivery directory (outside the source tree).
#
# usage: scripts/export-patches.sh [DELIVERY_DIR]      (default: ../tokenscope-patches)
set -eu
SRC=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-"$SRC/../tokenscope-patches"}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
case "$OUT" in "$SRC"|"$SRC"/*) echo "delivery dir must be outside the source repo" >&2; exit 1;; esac

rm -rf "$OUT/patches"; mkdir -p "$OUT/patches"
git -C "$SRC" format-patch --root --binary -N -o "$OUT/patches" >/dev/null
( cd "$OUT/patches" && ls -1 *.patch ) > "$OUT/series"
git -C "$SRC" log --reverse --no-merges --format='%H' > "$OUT/.commits"

STATUS="$OUT/verification.tsv"   # optional: <patch-filename><TAB><status>
{
  echo "# Patch manifest"
  echo
  echo "Ordered \`git format-patch --root\` series. Apply in exactly this order (see \`series\`)."
  echo "Hashes are those of the source repository; \`git am\` creates new commit hashes, so"
  echo "reconstruction is verified by per-commit **tree hash**, author and subject instead."
  echo
  echo "| # | Patch file | Source commit | Subject | Purpose | Verification |"
  echo "|---|------------|---------------|---------|---------|--------------|"
  n=0
  while read -r patch; do
    n=$((n+1))
    commit=$(sed -n "${n}p" "$OUT/.commits")
    subject=$(git -C "$SRC" log -1 --format=%s "$commit" | sed 's/|/\\|/g')
    purpose=$(git -C "$SRC" log -1 --format=%b "$commit" | tr '\n' ' ' | sed 's/  */ /g; s/ $//; s/|/\\|/g')
    [ -n "$purpose" ] || purpose="(see subject)"
    st="not verified"
    [ -f "$STATUS" ] && st=$(awk -F'\t' -v p="$patch" '$1==p{print $2}' "$STATUS") && [ -n "$st" ] || st="not verified"
    printf '| %03d | `%s` | `%s` | %s | %s | %s |\n' "$n" "$patch" "$(echo "$commit" | cut -c1-12)" "$subject" "$purpose" "$st"
  done < "$OUT/series"
} > "$OUT/PATCH_MANIFEST.md"
rm -f "$OUT/.commits"

cat > "$OUT/PATCHES_README.md" <<'R'
# Reconstructing TokenScope from patches

Patches do not create a repository; start from an empty directory.

## Prerequisites
* `git` >= 2.30 (`git am`), POSIX `sh`.
* To also build and test: Rust 1.99.0 (conda-forge `rust=1.99.0`), `cargo-build-sbf` 4.5.0, and
  Solana platform-tools **v1.58** extracted to `~/.cache/solana/v1.58/platform-tools`
  (see `docs/toolchain.md` inside the reconstructed repository).

## Commands
```sh
DELIVERY=/absolute/path/to/this/directory     # contains `series` and `patches/`
mkdir tokenscope-rebuilt && cd tokenscope-rebuilt
git init -b main
git config core.autocrlf false
git config user.name  "Your Name"             # `git am` needs a committer identity
git config user.email "you@example.com"
while read -r p; do git am "$DELIVERY/patches/$p" || git am --allow-empty || break; done < "$DELIVERY/series"
git log --oneline | wc -l                      # must equal the number of lines in `series`
```
The explicit `series` file defines order; do not rely on shell wildcard ordering.

## Verify
```sh
./scripts/verify-reconstruction.sh "$DELIVERY" --run-tests   # builds a fresh repo, compares
                                                              # tree hashes, runs fmt/clippy/tests
```
Each reconstructed commit has the same tree, author, author date and subject as the source commit;
only commit/committer metadata differs.
R
echo "exported $(wc -l < "$OUT/series") patches to $OUT"
