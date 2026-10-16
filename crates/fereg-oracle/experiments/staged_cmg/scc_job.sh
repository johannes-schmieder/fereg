#!/bin/bash -l
set -euo pipefail
mode=${1:?smoke or measure}
root=${2:?absolute campaign root}
cd "$root/source"
export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo
export RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$root/build"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
export LC_ALL=C TZ=UTC
if [[ "$mode" == smoke ]]; then
    cargo +1.85.1 test --locked
    cargo +1.85.1 build --locked --release
fi
python3 run_scc.py "$mode" --binary "$root/build/release/staged-cmg-prototype" --output "$root/$mode"
