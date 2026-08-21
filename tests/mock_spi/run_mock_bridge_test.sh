#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD="$ROOT/tests/mock_spi/build"
rm -rf "$BUILD"
mkdir -p "$BUILD"
"${CC:-cc}" \
  -std=c11 -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror \
  -DSYSTEM=OPUNIX \
  -I"$ROOT/plugin" \
  "$ROOT/plugin/stplugin.c" \
  "$ROOT/plugin/stata_bridge.c" \
  "$ROOT/tests/mock_spi/mock_spi.c" \
  -o "$BUILD/mock_spi_test"
"$BUILD/mock_spi_test"
