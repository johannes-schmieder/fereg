#!/bin/bash -l
set -euo pipefail
mode=${1:?smoke or measure}
root=${2:?absolute campaign root}
cd "$root/source"
module load miniconda/25.3.1
staged_python="${SCC_MINICONDA_BIN:?}/python3"
export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo
export RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$root/build"
export CARGO_BUILD_JOBS="${NSLOTS:?SGE slot allocation required}"
export RUST_TEST_THREADS="$CARGO_BUILD_JOBS"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
export LC_ALL=C TZ=UTC
if [[ "${STAGED_PROTOCOL:-original}" == persistent ]]; then
    export STAGED_PREVIOUS_BINARY="$root/previous/staged-cmg-prototype"
fi
"$staged_python" --version
if [[ "$mode" == smoke ]]; then
    cargo +1.85.1 test --locked
    cargo +1.85.1 build --locked --release
fi
"$staged_python" run_scc.py "$mode" --binary "$root/build/release/staged-cmg-prototype" --output "$root/$mode" --protocol "${STAGED_PROTOCOL:-original}"
