#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_ols_parents
#$ -cwd
#$ -j y
set -euo pipefail
module load python3/3.12.4 stata-mp/19
export STATA_MP_PATH="$(command -v stata-mp)"
export CARGO_HOME=$RUST_TOOLCHAINS/cargo
export RUSTUP_HOME=$RUST_TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 CARGO_NET_OFFLINE=true
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1 FEREG_BUILD_DATE=2026-09-16
cpus=$(python3 -c 'import os; print(",".join(map(str,sorted(os.sched_getaffinity(0))[:int(os.environ["NSLOTS"])])))')
exec taskset -c "$cpus" bash run-inner.sh
