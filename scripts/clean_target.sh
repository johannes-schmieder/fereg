#!/usr/bin/env bash
# Remove task outputs from target/, keeping only Cargo's build cache and the
# current replay baseline (AGENTS.md, repository hygiene). Without arguments it
# lists what would be removed; --apply deletes it. Run it only when no other
# task is using target/ (an A/B build, replay, timing run or worktree).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$ROOT/target"
apply=no
case "${1:-}" in
  "") ;;
  --apply) apply=yes ;;
  *)
    printf 'usage: %s [--apply]\n' "$0" >&2
    exit 2
    ;;
esac
[[ -d "$TARGET" && ! -L "$TARGET" ]] || exit 0

# Cargo profiles and target triples used by the build scripts, Cargo's own
# metadata, and the baseline plugin for exact replay checks.
keep=" debug ci release aarch64-apple-darwin x86_64-apple-darwin \
x86_64-unknown-linux-gnu x86_64-pc-windows-msvc replay-baseline \
.rustc_info.json CACHEDIR.TAG "
shopt -s dotglob nullglob
count=0
for entry in "$TARGET"/*; do
  name="${entry##*/}"
  [[ "$keep" == *" $name "* ]] && continue
  du -sh -- "$entry" 2>/dev/null || printf '?\t%s\n' "$entry"
  if [[ "$apply" == yes ]]; then
    rm -rf -- "$entry"
  fi
  count=$((count + 1))
done
if [[ "$apply" == yes ]]; then
  git -C "$ROOT" worktree prune
  printf 'removed %d entries; target/ is now %s\n' "$count" "$(du -sh "$TARGET" | cut -f1)"
else
  printf '%d entries would be removed; rerun with --apply\n' "$count"
fi
