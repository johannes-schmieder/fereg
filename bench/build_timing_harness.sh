#!/bin/bash
# Build the replay harness with a monotonic clock around the single native call.
# The ordinary replay/loader build is unchanged; REPLAY_TIMING only adds the
# clock and streams numeric output columns as raw binary beside the JSON.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-$ROOT/target/timing-harness/timing-harness}"
mkdir -p "$(dirname "$OUTPUT")"
case "$(uname -s)" in
  Darwin) SYSTEM_DEFINE=APPLEMAC; EXTRA_LIBS=(-lc) ;;
  Linux) SYSTEM_DEFINE=OPUNIX; EXTRA_LIBS=(-ldl) ;;
  *) printf 'unsupported Unix host for the timing harness\n' >&2; exit 2 ;;
esac
"${CC:-cc}" -std=c11 -O2 -Wall -Wextra -Wpedantic -Wconversion -Wshadow \
  -Wstrict-prototypes -Wmissing-prototypes -Werror \
  -D_POSIX_C_SOURCE=200809L -DREPLAY_TIMING -DSYSTEM="$SYSTEM_DEFINE" \
  -I"$ROOT/plugin" "$ROOT/tests/mock_spi/actual_plugin_loader_test.c" \
  "${EXTRA_LIBS[@]}" -o "$OUTPUT"
printf '%s\n' "$OUTPUT"
