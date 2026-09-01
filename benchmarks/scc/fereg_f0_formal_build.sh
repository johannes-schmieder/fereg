#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT=${1:?exact extracted source root is required}
TARGET=x86_64-unknown-linux-gnu
BUILD="$CARGO_TARGET_DIR/native-linux-c"
DIST="$SOURCE_ROOT/dist"
mkdir -p "$BUILD" "$DIST"
rm -f "$BUILD"/*.o "$DIST/fereg_linux.plugin"

cargo build --locked --offline --release -p fereg-plugin --target "$TARGET" \
  --manifest-path "$SOURCE_ROOT/Cargo.toml"

COMMON_CFLAGS=(-std=c11 -O3 -fPIC -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror -DSYSTEM=OPUNIX -I"$SOURCE_ROOT/plugin")
"$CC" "${COMMON_CFLAGS[@]}" -c "$SOURCE_ROOT/plugin/stplugin.c" -o "$BUILD/stplugin.o"
"$CC" "${COMMON_CFLAGS[@]}" -c "$SOURCE_ROOT/plugin/stata_bridge.c" -o "$BUILD/stata_bridge.o"
"$CC" -shared -Wl,-z,relro,-z,now,--no-undefined \
  -Wl,--version-script="$SOURCE_ROOT/plugin/exports_linux.map" \
  -Wl,--exclude-libs,ALL "$BUILD/stplugin.o" "$BUILD/stata_bridge.o" \
  "$CARGO_TARGET_DIR/$TARGET/release/libfereg_plugin.a" \
  -ldl -lpthread -lm -lrt -lutil -o "$DIST/fereg_linux.plugin"

python3 "$SOURCE_ROOT/scripts/audit_binary.py" "$DIST/fereg_linux.plugin"
"$SOURCE_ROOT/tests/mock_spi/run_actual_unix_plugin_test.sh" "$DIST/fereg_linux.plugin"
sha256sum "$DIST/fereg_linux.plugin" > "$DIST/fereg_linux.plugin.sha256"
printf 'Built %s\n' "$DIST/fereg_linux.plugin"
