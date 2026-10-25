#!/bin/sh
# Clean-room reconstruction check.
#
# usage: scripts/verify-reconstruction.sh DELIVERY_DIR [--run-tests] [--source REPO]
# Builds a brand-new repository in a temp dir, applies every patch listed in DELIVERY_DIR/series
# with `git am`, then compares it with the source repository commit-by-commit (tree hash, author,
# subject) and file-by-file. With --run-tests it also builds the programs and runs fmt/clippy/tests
# inside the reconstructed repository.
set -eu
DELIVERY=$(cd "$1" && pwd); shift
RUN_TESTS=0
SRC=$(cd "$(dirname "$0")/.." && pwd)
while [ $# -gt 0 ]; do
  case "$1" in
    --run-tests) RUN_TESTS=1;;
    --source) shift; SRC=$(cd "$1" && pwd);;
    *) echo "unknown option $1" >&2; exit 2;;
  esac
  shift
done
WORK=${TOKENSCOPE_RECON_DIR:-$(mktemp -d)}
rm -rf "$WORK"; mkdir -p "$WORK"
cd "$WORK"
git init -q -b main
git config user.name "Reconstruction"; git config user.email "recon@tokenscope.invalid"
count=0
while read -r p; do
  git am -q "$DELIVERY/patches/$p" || { echo "FAIL: git am failed on $p" >&2; exit 1; }
  count=$((count+1))
done < "$DELIVERY/series"
want=$(wc -l < "$DELIVERY/series"); src_count=$(git -C "$SRC" rev-list --count HEAD)
[ "$count" -eq "$want" ] && [ "$count" -eq "$src_count" ] || { echo "FAIL: commit count $count / series $want / source $src_count" >&2; exit 1; }

fmt='%T %an <%ae> %aI %s'
git log --reverse --format="$fmt" > "$WORK/../recon-history.txt"
git -C "$SRC" log --reverse --format="$fmt" > "$WORK/../src-history.txt"
if ! diff -u "$WORK/../src-history.txt" "$WORK/../recon-history.txt"; then
  echo "FAIL: history differs (tree/author/date/subject)" >&2; exit 1
fi
echo "OK: $count commits reapplied; tree hash, author, author date and subject match for every commit"
if ! diff -r -x .git -x target "$SRC" "$WORK" >/dev/null 2>&1; then
  # Compare tracked content only (the source tree may hold ignored build output).
  git -C "$SRC" ls-files | sort > "$WORK/../src-files.txt"
  git ls-files | sort > "$WORK/../recon-files.txt"
  diff -u "$WORK/../src-files.txt" "$WORK/../recon-files.txt" || { echo "FAIL: file lists differ" >&2; exit 1; }
  for f in $(cat "$WORK/../src-files.txt"); do cmp -s "$SRC/$f" "$WORK/$f" || { echo "FAIL: $f differs" >&2; exit 1; }; done
fi
echo "OK: reconstructed working tree matches the source repository"
[ -z "$(git status --porcelain)" ] || { echo "FAIL: reconstructed tree is dirty" >&2; exit 1; }
echo "reconstructed at: $WORK"
if [ "$RUN_TESTS" -eq 1 ]; then
  ./scripts/build-programs.sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  echo "OK: fmt, clippy and tests passed in the reconstructed repository"
fi
