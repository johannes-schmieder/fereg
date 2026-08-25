#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="$ROOT/target/portable-static-checks"
PYTHON_BIN="${FEREG_PYTHON_BIN:-python3}"
rm -rf "$BUILD"
mkdir -p "$BUILD"

while IFS= read -r script; do
  bash -n "$script"
done < <(find "$ROOT/scripts" "$ROOT/tests/mock_spi" -type f -name '*.sh' -print | sort)
printf '%s\n' 'portable check: shell syntax PASS'

PYTHONPYCACHEPREFIX="$BUILD/pycache" "$PYTHON_BIN" -m compileall -q \
  "$ROOT/scripts" "$ROOT/tests/python"
printf '%s\n' 'portable check: Python compile PASS'

"$PYTHON_BIN" - "$ROOT" <<'PY'
from pathlib import Path
import sys
import tomllib
root = Path(sys.argv[1])
for path in [root / "Cargo.toml", root / "references.lock.toml", root / "rust-toolchain.toml", *sorted((root / "crates").glob("*/Cargo.toml"))]:
    with path.open("rb") as handle:
        tomllib.load(handle)
print("portable check: TOML parse PASS")
PY

compile_pair() {
  local compiler="$1"
  local system="$2"
  local tag="$3"
  local flags=(-std=c11 -O2 -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Wstrict-prototypes -Wmissing-prototypes -Werror -DSYSTEM="$system" -I"$ROOT/plugin")
  "$compiler" "${flags[@]}" -c "$ROOT/plugin/stplugin.c" -o "$BUILD/stplugin.$tag.o"
  "$compiler" "${flags[@]}" -c "$ROOT/plugin/stata_bridge.c" -o "$BUILD/stata_bridge.$tag.o"
}

CC_BIN="${CC:-cc}"
compile_pair "$CC_BIN" OPUNIX cc-unix
compile_pair "$CC_BIN" APPLEMAC cc-macos
printf 'portable check: strict C via %s PASS\n' "$CC_BIN"

if command -v clang >/dev/null 2>&1 && [[ "$(command -v clang)" != "$(command -v "$CC_BIN")" ]]; then
  compile_pair clang OPUNIX clang-unix
  compile_pair clang APPLEMAC clang-macos
  printf '%s\n' 'portable check: strict C via clang PASS'
fi

COMPILER_MACROS="$($CC_BIN -dM -E -x c /dev/null 2>/dev/null || true)"
if grep -q '__GNUC__' <<<"$COMPILER_MACROS" && ! grep -q '__clang__' <<<"$COMPILER_MACROS"; then
  "$CC_BIN" -std=c11 -O0 -fanalyzer -Wall -Wextra -Wpedantic -Wconversion -Wshadow \
    -Wstrict-prototypes -Wmissing-prototypes -Werror -DSYSTEM=OPUNIX -I"$ROOT/plugin" \
    -c "$ROOT/plugin/stata_bridge.c" -o "$BUILD/stata_bridge.analyzer.o"
  printf '%s\n' 'portable check: GCC static analyzer PASS'
fi
