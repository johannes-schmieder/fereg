#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="$ROOT/target/native-macos"
DIST="$ROOT/dist"
mkdir -p "$BUILD" "$DIST"
rm -f "$BUILD"/* "$DIST/stfixest_macos.plugin"

export STFIXEST_BUILD_DATE="${STFIXEST_BUILD_DATE:-$(date -u +%Y-%m-%d)}"
export STFIXEST_GIT_COMMIT="${STFIXEST_GIT_COMMIT:-$(git -C "$ROOT" rev-parse --short=12 HEAD 2>/dev/null || printf unknown)}"

rustup target add x86_64-apple-darwin aarch64-apple-darwin

build_slice() {
  local rust_target="$1"
  local clang_target="$2"
  local minimum="$3"
  local suffix="$4"
  local deployment_target="${minimum#*=}"
  MACOSX_DEPLOYMENT_TARGET="$deployment_target" cargo build --locked --release -p stfixest-plugin --target "$rust_target"
  clang -std=c11 -O3 -fPIC -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror -DSYSTEM=APPLEMAC \
    -target "$clang_target" "$minimum" -I"$ROOT/plugin" \
    -c "$ROOT/plugin/stplugin.c" -o "$BUILD/stplugin.$suffix.o"
  clang -std=c11 -O3 -fPIC -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror -DSYSTEM=APPLEMAC \
    -target "$clang_target" "$minimum" -I"$ROOT/plugin" \
    -c "$ROOT/plugin/stata_bridge.c" -o "$BUILD/stata_bridge.$suffix.o"
  clang -bundle -target "$clang_target" "$minimum" \
    -Wl,-dead_strip \
    -Wl,-exported_symbols_list,"$ROOT/plugin/exports_macos.txt" \
    "$BUILD/stplugin.$suffix.o" "$BUILD/stata_bridge.$suffix.o" \
    "$ROOT/target/$rust_target/release/libstfixest_plugin.a" \
    -framework Security -framework CoreFoundation -framework SystemConfiguration \
    -liconv -lresolv \
    -o "$BUILD/stfixest_macos.$suffix.plugin"
}

build_slice x86_64-apple-darwin x86_64-apple-macos10.13 -mmacosx-version-min=10.13 x86_64
build_slice aarch64-apple-darwin arm64-apple-macos11 -mmacosx-version-min=11.0 arm64

lipo -create \
  "$BUILD/stfixest_macos.x86_64.plugin" \
  "$BUILD/stfixest_macos.arm64.plugin" \
  -output "$DIST/stfixest_macos.plugin"

# Ad-hoc signing is deterministic enough for local loading and avoids an
# unsigned universal bundle on hardened macOS hosts. Release signing may replace it.
codesign --force --sign - "$DIST/stfixest_macos.plugin"

python3 "$ROOT/scripts/audit_binary.py" "$DIST/stfixest_macos.plugin"
"$ROOT/tests/mock_spi/run_actual_unix_plugin_test.sh" "$DIST/stfixest_macos.plugin"
shasum -a 256 "$DIST/stfixest_macos.plugin" > "$DIST/stfixest_macos.plugin.sha256"
printf 'Built universal %s\n' "$DIST/stfixest_macos.plugin"
