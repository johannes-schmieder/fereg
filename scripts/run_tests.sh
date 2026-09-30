#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v cargo >/dev/null 2>&1 || {
  printf '%s\n' 'Rust/Cargo is required; install the pinned rust-toolchain.toml toolchain.' >&2
  exit 127
}

profile="${FEREG_TEST_PROFILE:-ci}"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features --profile "$profile" -- -D warnings
cargo clippy -p fereg-plugin --all-targets --features ffi --profile "$profile" -- -D warnings
cargo test --workspace --all-targets --no-default-features --profile "$profile"
# Accepted-state authority includes a compiler-enforced mutation contract.
cargo test -p fereg-engine --doc --profile "$profile"
# The FFI allocator observes every test's allocations, even outside an audit.
cargo test -p fereg-plugin --features ffi --lib --profile "$profile" -- --test-threads=1
# Exercise the explicitly isolated allocator audits as well.
cargo test -p fereg-plugin --features ffi --lib --profile "$profile" -- --ignored --test-threads=1
if [[ -f scripts/check_sizes.sh ]]; then
  bash scripts/check_sizes.sh
fi
tests/mock_spi/run_mock_bridge_test.sh
if [[ "$(uname -s)" == Linux ]]; then
  tests/mock_spi/run_mock_linux_plugin_audit.sh
else
  printf '%s\n' 'SKIP: Linux mock-plugin audit requires a GNU ELF linker.'
fi
