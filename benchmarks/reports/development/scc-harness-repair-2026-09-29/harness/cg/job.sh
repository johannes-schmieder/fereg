#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_cg_span
#$ -pe omp 8
#$ -l mem_per_core=3G,h_rt=01:00:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260929-nonretaining-cg-02 && "$NSLOTS" == 8 ]]
module purge
module load python3/3.12.4
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=8
export PATH="$CARGO_HOME/bin:$PATH"
export RAYON_NUM_THREADS=8 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
python3 "$RUN_ROOT/run.py" --run-root "$RUN_ROOT"
