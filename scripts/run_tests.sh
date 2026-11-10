#!/usr/bin/env bash
set -euo pipefail
package_root=""
if [[ "$#" -eq 0 ]]; then
  source_only=false
elif [[ "$#" -eq 1 && "$1" == --source-only ]]; then
  source_only=true
elif [[ "$#" -eq 2 && "$1" == --package-root && -d "$2" ]]; then
  # The strict path, with the installed-package checker on a staged
  # candidate root package instead of the repository's own dist/.
  source_only=false
  package_root="$(cd "$2" && pwd)"
else
  printf '%s\n' 'Usage: scripts/run_tests.sh [--source-only | --package-root STAGED_ROOT_PACKAGE]' >&2
  exit 2
fi
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# One interpreter runs every Python check, including the nested scripts.
python="${FEREG_PYTHON_BIN:-python3}"
export FEREG_PYTHON_BIN="$python"
if [[ "$source_only" == true ]]; then
  printf '%s\n' \
    'SOURCE-ONLY: NOT PERFORMED: installed-package binding, inventory, hash, ABI and Mata checks.' \
    'SOURCE-ONLY: this run does not qualify a package for release or promotion.'
fi
if [[ -n "$package_root" ]]; then
  printf 'PACKAGE: checking the staged candidate %s, not the repository dist/.\n' "$package_root"
fi
# Stop before any check when the scientific modules could not run.
"$python" scripts/check_python_environment.py
"$python" -m unittest discover -s tests/python
if [[ -n "$package_root" ]]; then
  "$python" scripts/check_github_install.py --package-root "$package_root"
elif [[ "$source_only" == false ]]; then
  "$python" scripts/check_github_install.py
fi
"$python" scripts/check_fma_rounding_vectors.py
"$python" scripts/check_fe_boundary_response_oracle.py
"$python" scripts/check_fe_cancellation_oracle.py

command -v cargo >/dev/null 2>&1 || {
  printf '%s\n' 'Rust/Cargo is required; install the pinned rust-toolchain.toml toolchain.' >&2
  exit 127
}

profile="${FEREG_TEST_PROFILE:-ci}"

cargo fmt --all -- --check
# The fuzz package is its own workspace; keep its targets compiling.
(cd fuzz && cargo fmt -- --check && cargo check --locked --quiet)
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
bash scripts/run_portable_static_checks.sh
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
if [[ "$source_only" == true ]]; then
  printf '%s\n' 'SOURCE-ONLY: source integration PASS; installed-package admission NOT PERFORMED; release/promotion blocked.'
fi
