#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ $# != 2 ]]; then
  printf 'usage: %s NEXT_PLUGIN BASELINE_PLUGIN\n' "$0" >&2
  exit 2
fi
# Regenerate only this task-owned directory; stale or missing cases fail the matrix check.
mkdir -p target/replay/cases
cargo run -p fereg-oracle --example replay --profile ci -- target/replay/cases
./tests/mock_spi/run_actual_unix_plugin_test.sh "$1"
python3 scripts/replay_plugins.py --next "$1" --baseline "$2"
