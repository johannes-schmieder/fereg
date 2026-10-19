#!/usr/bin/env bash
# Diagnostic only: export a PRIVATE source snapshot for one arm of the paired
# M3a admission/terminal comparison, with the bounded Auto route trace that
# appends stored CMG hierarchy metadata to installed `auto_schur_route` records.
#
#   pre-m3a  fereg 7ccab5a9 (CMG d11b2ac4) plus
#            bench/diagnostics/pre-m3a-route-hierarchy-trace.patch
#   m3a REV  a fereg revision that already emits the metadata (no patch)
#
# The snapshot is a plain directory made with `git archive`: no branch, ref,
# worktree or history is created or changed. The patch only formats metadata
# the build already stored, under the existing FEREG_SOLVER_TRACE opt-in; it
# changes no arithmetic, admission, routing, resource handling, cancellation
# or public result. Build the snapshot with its own scripts/build_linux.sh or
# scripts/build_macos.sh and the printed FEREG_GIT_COMMIT label, so the binary
# identifies itself as a diagnostic build. Never publish or install it.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PRE_M3A=7ccab5a901b135e59b73603426ea2b8bfb128d89
PRE_M3A_CMG=d11b2ac46de73b6e3d5e37db84f816e3a8cccb6d
PATCH="$ROOT/bench/diagnostics/pre-m3a-route-hierarchy-trace.patch"
usage() {
  printf 'usage: %s pre-m3a OUT_DIR\n       %s m3a REV OUT_DIR\n' "$0" "$0" >&2
  exit 2
}
case "${1:-}" in
  pre-m3a) [[ $# == 2 ]] || usage; revision=$PRE_M3A; out=$2 ;;
  m3a) [[ $# == 3 ]] || usage; revision=$(git -C "$ROOT" rev-parse --verify "$2^{commit}"); out=$3 ;;
  *) usage ;;
esac
[[ ! -e "$out" ]] || { printf '%s already exists\n' "$out" >&2; exit 1; }
git -C "$ROOT" cat-file -e "$revision^{commit}"
mkdir -p "$out"
git -C "$ROOT" archive "$revision" | tar -x -C "$out"
fe="$out/crates/fereg-core/src/fe"
if [[ $1 == pre-m3a ]]; then
  grep -q "rev = \"$PRE_M3A_CMG\"" "$out/crates/fereg-core/Cargo.toml"
  grep -q "rev=$PRE_M3A_CMG#$PRE_M3A_CMG" "$out/Cargo.lock"
  patch -d "$out" -p1 --forward --batch --no-backup-if-mismatch < "$PATCH"
fi
# Both arms must emit the same observation from the same stored metadata.
grep -q 'pub(super) fn trace_hierarchy(&self) -> String' "$fe/cmg_schur.rs"
grep -q 'refusal={:?}{hierarchy}' "$fe/borrowed_map.rs"
label="$(printf '%.12s' "$revision")-route-trace-diagnostic"
printf 'DIAGNOSTIC BUILD ONLY: %s arm, fereg %s%s.\nBuild with FEREG_GIT_COMMIT=%s; never publish or install.\n' \
  "$1" "$revision" "$([[ $1 == pre-m3a ]] && printf ' + route-hierarchy trace patch')" "$label" \
  > "$out/DIAGNOSTIC_BUILD.txt"
cat "$out/DIAGNOSTIC_BUILD.txt"
