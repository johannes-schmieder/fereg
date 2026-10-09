#!/bin/bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
rustup_bin="${RUSTUP_BIN:-$(command -v rustup || printf '%s' /opt/homebrew/bin/rustup)}"
# The pinned channel lives in rust-toolchain.toml; RUST_TOOLCHAIN overrides it.
pinned="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo_root/rust-toolchain.toml")"
toolchain="${RUST_TOOLCHAIN:-$pinned}"

if [[ ! -x "$rustup_bin" ]]; then
  echo "Rust quick check could not find rustup at $rustup_bin" >&2
  exit 127
fi
if ! "$rustup_bin" toolchain list | grep -Eq "^${toolchain}(-|[[:space:]])"; then
  echo "Required Rust toolchain $toolchain is not installed" >&2
  exit 127
fi

toolchain_root="$("$rustup_bin" run "$toolchain" rustc --print sysroot)"
export PATH="$toolchain_root/bin:/opt/homebrew/bin:$PATH"
export CARGO_TERM_COLOR=always
export RUST_BACKTRACE=1
export PYTHONPYCACHEPREFIX="${RUNNER_TEMP:-/private/tmp}/fereg-python-cache"

cd "$repo_root"
./scripts/run_tests.sh
