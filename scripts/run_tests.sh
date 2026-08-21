#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 scripts/check_repository.py
python3 -m unittest discover -s tests/python -v
scripts/run_portable_static_checks.sh
tests/mock_spi/run_mock_bridge_test.sh
if [[ "$(uname -s)" == "Linux" ]]; then
  tests/mock_spi/run_mock_linux_plugin_audit.sh
else
  printf '%s\n' 'SKIP: Linux mock-plugin audit requires a GNU ELF linker.'
fi

if command -v cargo >/dev/null 2>&1; then
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --no-default-features -- -D warnings
  cargo test --workspace --all-targets --no-default-features
  cargo test -p stfixest-plugin --features ffi --lib
else
  printf '%s\n' 'SKIP: Rust/Cargo not installed; see VALIDATION.md.'
fi
