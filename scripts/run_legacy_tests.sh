#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ -n "${FEREG_PYTHON_BIN:-}" ]]; then
  PYTHON_BIN="$FEREG_PYTHON_BIN"
else
  PYTHON_BIN=""
  for candidate in python3 python3.14 python3.13 python3.12 python3.11; do
    if command -v "$candidate" >/dev/null 2>&1 &&
      "$candidate" -c 'import tomllib' >/dev/null 2>&1; then
      PYTHON_BIN="$(command -v "$candidate")"
      break
    fi
  done
fi
if [[ -z "$PYTHON_BIN" ]] || ! "$PYTHON_BIN" -c 'import tomllib' >/dev/null 2>&1; then
  printf '%s\n' 'fereg tests require Python 3.11+ with tomllib' >&2
  exit 127
fi

"$PYTHON_BIN" scripts/check_repository.py
"$PYTHON_BIN" -m unittest discover -s tests/python -v
FEREG_PYTHON_BIN="$PYTHON_BIN" scripts/run_portable_static_checks.sh
tests/mock_spi/run_mock_bridge_test.sh
if [[ "$(uname -s)" == "Linux" ]]; then
  tests/mock_spi/run_mock_linux_plugin_audit.sh
else
  printf '%s\n' 'SKIP: Linux mock-plugin audit requires a GNU ELF linker.'
fi

if command -v cargo >/dev/null 2>&1; then
  cargo fmt --all -- --check
  cargo fmt --manifest-path fuzz/Cargo.toml --all -- --check
  cargo clippy --workspace --all-targets --no-default-features -- -D warnings
  cargo test --workspace --all-targets --no-default-features
  cargo test -p fereg-plugin --features ffi --lib
else
  printf '%s\n' 'SKIP: Rust/Cargo not installed; see STATUS.md.'
fi
