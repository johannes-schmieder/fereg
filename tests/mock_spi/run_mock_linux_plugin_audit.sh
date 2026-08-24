#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD="$ROOT/tests/mock_spi/build/plugin-audit"
CC_BIN="${CC:-cc}"
rm -rf "$BUILD"
mkdir -p "$BUILD"

CFLAGS=(-std=c11 -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror -O2 -fPIC -DSYSTEM=OPUNIX -I"$ROOT/plugin")
"$CC_BIN" "${CFLAGS[@]}" -c "$ROOT/plugin/stplugin.c" -o "$BUILD/stplugin.o"
"$CC_BIN" "${CFLAGS[@]}" -c "$ROOT/plugin/stata_bridge.c" -o "$BUILD/stata_bridge.o"
"$CC_BIN" "${CFLAGS[@]}" -c "$ROOT/tests/mock_spi/dummy_dispatch.c" -o "$BUILD/dummy_dispatch.o"
"$CC_BIN" -shared -Wl,-z,relro,-z,now,--no-undefined \
  -Wl,--version-script="$ROOT/plugin/exports_linux.map" \
  "$BUILD/stplugin.o" "$BUILD/stata_bridge.o" "$BUILD/dummy_dispatch.o" \
  -o "$BUILD/fereg_mock_linux.plugin"

"$CC_BIN" -std=c11 -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror \
  -DSYSTEM=OPUNIX -I"$ROOT/plugin" \
  "$ROOT/tests/mock_spi/plugin_loader_test.c" -ldl -o "$BUILD/plugin_loader_test"
python3 "$ROOT/scripts/audit_binary.py" "$BUILD/fereg_mock_linux.plugin" > "$BUILD/audit.json"
"$BUILD/plugin_loader_test" "$BUILD/fereg_mock_linux.plugin"

# Validate the target-native actual-Rust harness against a behaviorally faithful
# C dispatcher so harness defects are caught even when Rust is unavailable.
"$CC_BIN" "${CFLAGS[@]}" -c "$ROOT/tests/mock_spi/fake_actual_dispatch.c" -o "$BUILD/fake_actual_dispatch.o"
"$CC_BIN" -shared -Wl,-z,relro,-z,now,--no-undefined \
  -Wl,--version-script="$ROOT/plugin/exports_linux.map" \
  "$BUILD/stplugin.o" "$BUILD/stata_bridge.o" "$BUILD/fake_actual_dispatch.o" \
  -o "$BUILD/fereg_fake_actual.plugin"
"$CC_BIN" -std=c11 -Wall -Wextra -Wpedantic -Wconversion -Wshadow \
  -Wstrict-prototypes -Wmissing-prototypes -Werror -DSYSTEM=OPUNIX \
  -I"$ROOT/plugin" "$ROOT/tests/mock_spi/actual_plugin_loader_test.c" -ldl \
  -o "$BUILD/actual_plugin_loader_test"
"$BUILD/actual_plugin_loader_test" "$BUILD/fereg_fake_actual.plugin"
