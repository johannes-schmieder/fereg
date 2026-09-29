#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_solver_public
#$ -pe omp 16
#$ -l mem_per_core=3G,h_rt=00:45:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${NSLOTS:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260929-solver-public-02 && "$NSLOTS" == 16 ]]
module purge
module load python3/3.12.4 stata-mp/19
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=8 RUSTFLAGS=""
unset CARGO_ENCODED_RUSTFLAGS
export PATH="$CARGO_HOME/bin:$PATH"
export RAYON_NUM_THREADS=16 OMP_NUM_THREADS=16 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
cpu_set=$(python3 -c 'import os; n=int(os.environ["NSLOTS"]); a=sorted(os.sched_getaffinity(0)); assert len(a)>=n; print(",".join(map(str,a[:n])))')
taskset -pc "$cpu_set" "$$"
cd "$RUN_ROOT"
python3 -B -m unittest -v test_run > harness-tests.txt 2>&1
python3 -B run.py "$RUN_ROOT"
