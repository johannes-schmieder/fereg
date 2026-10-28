#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ $# -lt 1 || $# -gt 2 ]]; then
  printf 'usage: %s PLUGIN [BASELINE_PLUGIN]\n' "$0" >&2
  exit 2
fi
# Regenerate only this task-owned directory; stale or missing cases fail the matrix check.
REPLAY_ROOT="${FEREG_REPLAY_ROOT:-target/replay}"
mkdir -p "$REPLAY_ROOT/cases"
cargo run -p fereg-oracle --example replay --profile ci -- "$REPLAY_ROOT/cases"
./tests/mock_spi/run_actual_unix_plugin_test.sh "$1"
arguments=(--plugin "$1" --cases "$REPLAY_ROOT/cases" --output "$REPLAY_ROOT/results")
if [[ $# == 2 ]]; then arguments+=(--baseline "$2"); fi
# Space-separated affected-scope files (their disjoint union); requires a baseline.
if [[ -n "${FEREG_REPLAY_AFFECTED_SCOPES:-}" ]]; then
  read -r -a scopes <<< "$FEREG_REPLAY_AFFECTED_SCOPES"
  arguments+=(--affected-scope "${scopes[@]}")
fi
if [[ -n "${FEREG_REPLAY_BASELINE_FIXES+x}" ]]; then
  arguments+=(--baseline-fixes "$FEREG_REPLAY_BASELINE_FIXES")
fi
python3 scripts/replay_plugins.py "${arguments[@]}"
