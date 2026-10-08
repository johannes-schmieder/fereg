#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 -m unittest discover -s tests/python -p test_plugin_replay.py
python3 scripts/check_fma_rounding_vectors.py

command -v cargo >/dev/null 2>&1 || {
  printf '%s\n' 'Rust/Cargo is required; install the pinned rust-toolchain.toml toolchain.' >&2
  exit 127
}

profile="${FEREG_TEST_PROFILE:-ci}"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features --profile "$profile" -- -D warnings
cargo clippy -p fereg-plugin --all-targets --features ffi --profile "$profile" -- -D warnings
cargo test --workspace --all-targets --no-default-features --profile "$profile"
# The FFI allocator observes every test's allocations, even outside an audit.
cargo test -p fereg-plugin --features ffi --lib --profile "$profile" -- --test-threads=1
# Serial test threads still share counters and surviving worker allocations.
# Each process-global audit needs a fresh process, not only a serial runner.
audit_list="$(cargo test -p fereg-plugin --features ffi --lib --profile "$profile" -- --ignored --list)"
audit_count=0
while IFS= read -r audit; do
  case "$audit" in
    *": test")
      audit_count=$((audit_count + 1))
      cargo test -p fereg-plugin --features ffi --lib --profile "$profile" "${audit%: test}" -- --ignored --exact --test-threads=1 --show-output
      ;;
  esac
done <<< "$audit_list"
if [[ "$audit_count" -eq 0 ]]; then
  printf '%s\n' 'No allocator audits discovered; refusing an empty audit gate.' >&2
  exit 1
fi
if [[ -f scripts/check_sizes.sh ]]; then
  bash scripts/check_sizes.sh
fi
tests/mock_spi/run_mock_bridge_test.sh
bash tests/mock_spi/run_numeric_batch_guard_test.sh
if [[ "$(uname -s)" == Linux ]]; then
  tests/mock_spi/run_mock_linux_plugin_audit.sh
else
  printf '%s\n' 'SKIP: Linux mock-plugin audit requires a GNU ELF linker.'
fi
