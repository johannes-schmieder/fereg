#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="x86_64-unknown-linux-gnu"
BUILD="$ROOT/target/native-linux"
CARGO_TARGET_ROOT="${CARGO_TARGET_DIR:-$ROOT/target}"
DIST="$ROOT/dist"
CC_BIN="${CC:-cc}"
mkdir -p "$BUILD" "$DIST"
rm -f "$BUILD"/*.o "$DIST/fereg_linux.plugin"

export FEREG_BUILD_DATE="${FEREG_BUILD_DATE:-$(date -u +%Y-%m-%d)}"
export FEREG_GIT_COMMIT="${FEREG_GIT_COMMIT:-$(git -C "$ROOT" rev-parse --short=12 HEAD 2>/dev/null || printf unknown)}"

cargo build --locked --release -p fereg-plugin --target "$TARGET"

COMMON_CFLAGS=(-std=c11 -O3 -fPIC -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror -DSYSTEM=OPUNIX -I"$ROOT/plugin")
"$CC_BIN" "${COMMON_CFLAGS[@]}" -c "$ROOT/plugin/stplugin.c" -o "$BUILD/stplugin.o"
"$CC_BIN" "${COMMON_CFLAGS[@]}" -c "$ROOT/plugin/stata_bridge.c" -o "$BUILD/stata_bridge.o"

"$CC_BIN" -shared \
  -Wl,-z,relro,-z,now,--no-undefined \
  -Wl,--version-script="$ROOT/plugin/exports_linux.map" \
  -Wl,--exclude-libs,ALL \
  "$BUILD/stplugin.o" "$BUILD/stata_bridge.o" \
  "$CARGO_TARGET_ROOT/$TARGET/release/libfereg_plugin.a" \
  -ldl -lpthread -lm -lrt -lutil \
  -o "$DIST/fereg_linux.plugin"

python3 "$ROOT/scripts/audit_binary.py" "$DIST/fereg_linux.plugin"
"$ROOT/tests/mock_spi/run_actual_unix_plugin_test.sh" "$DIST/fereg_linux.plugin"
sha256sum "$DIST/fereg_linux.plugin" > "$DIST/fereg_linux.plugin.sha256"
printf 'Built %s\n' "$DIST/fereg_linux.plugin"
