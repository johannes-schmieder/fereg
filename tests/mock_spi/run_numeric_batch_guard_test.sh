#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD="$ROOT/target/numeric-batch-guard"
mkdir -p "$BUILD"
for system in APPLEMAC OPUNIX; do
  "${CC:-cc}" -std=c11 -O2 -Wall -Wextra -Wpedantic -Wconversion -Wshadow \
    -Wstrict-prototypes -Wmissing-prototypes -Werror \
    -fsanitize=undefined -fno-sanitize-recover=all -DSYSTEM="$system" \
    -I"$ROOT/plugin" "$ROOT/plugin/stplugin.c" "$ROOT/plugin/stata_bridge.c" \
    "$ROOT/tests/mock_spi/numeric_batch_guard_test.c" -pthread -o "$BUILD/test-$system"
  "$BUILD/test-$system"
done
