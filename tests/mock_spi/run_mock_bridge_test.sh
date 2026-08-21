#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD="$ROOT/tests/mock_spi/build"
rm -rf "$BUILD"
mkdir -p "$BUILD"

PGINIT_DEFINITIONS="$(grep -hE '^[[:space:]]*STDLL[[:space:]]+pginit[[:space:]]*\(' \
  "$ROOT/plugin/stplugin.c" "$ROOT/plugin/stata_bridge.c" | wc -l | tr -d ' ')"
if [[ "$PGINIT_DEFINITIONS" != "1" ]] || \
   ! grep -Eq '^[[:space:]]*STDLL[[:space:]]+pginit[[:space:]]*\(' "$ROOT/plugin/stplugin.c"; then
  printf '%s\n' 'pginit must have exactly one definition in official stplugin.c' >&2
  exit 1
fi

"${CC:-cc}" \
  -std=c11 -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror \
  -DSYSTEM=OPUNIX \
  -I"$ROOT/plugin" \
  "$ROOT/plugin/stplugin.c" \
  "$ROOT/plugin/stata_bridge.c" \
  "$ROOT/tests/mock_spi/mock_spi.c" \
  -pthread \
  -o "$BUILD/mock_spi_test"
"$BUILD/mock_spi_test"
