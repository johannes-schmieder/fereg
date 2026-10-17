#!/bin/bash
set -euo pipefail
if [[ $# -ne 1 ]]; then
  printf 'usage: %s PATH_TO_PLUGIN\n' "$0" >&2
  exit 2
fi
# Cap actual ABI selftest requests, not just the compiler or BLAS environment.
# Unset preserves the local 32-worker check; verbose uses min(2, cap).
case "${FEREG_TEST_THREADS-32}" in
  [123456789]|[12][0123456789]|3[012]) ;;
  *)
    printf 'FEREG_TEST_THREADS must be an integer from 1 to 32 without leading zeros\n' >&2
    exit 2
    ;;
esac
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
BUILD="$ROOT/target/actual-plugin-harness"
mkdir -p "$BUILD"
case "$(uname -s)" in
  Darwin)
    SYSTEM_DEFINE=APPLEMAC
    EXTRA_LIBS=(-lc)
    ;;
  Linux)
    SYSTEM_DEFINE=OPUNIX
    EXTRA_LIBS=(-ldl)
    ;;
  *)
    printf 'unsupported Unix host for actual plugin harness\n' >&2
    exit 2
    ;;
esac
"${CC:-cc}" -std=c11 -O2 -Wall -Wextra -Wpedantic -Wconversion -Wshadow \
  -Wstrict-prototypes -Wmissing-prototypes -Werror \
  -DSYSTEM="$SYSTEM_DEFINE" -I"$ROOT/plugin" \
  "$ROOT/tests/mock_spi/actual_plugin_loader_test.c" "${EXTRA_LIBS[@]}" \
  -o "$BUILD/actual_plugin_loader_test"
"$BUILD/actual_plugin_loader_test" "$PLUGIN"
