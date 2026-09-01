#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_f0_setup
#$ -pe omp 16
#$ -l h_rt=04:00:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?RUN_ROOT is required}"
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/runs/[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "RUN_ROOT is not an exact fereg run directory" >&2
    exit 2
fi

module --force purge
module load stata-mp/19
module load python3/3.12.4

export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export PATH="$CARGO_HOME/bin:$PATH"
test "$(rustc --version)" = "rustc 1.85.1 (4eb161250 2025-03-15)"
test "$(cargo --version)" = "cargo 1.85.1 (d73d2caf9 2024-12-31)"

export OMP_NUM_THREADS="${NSLOTS:-1}"
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1

python3 -I -B -S "$RUN_ROOT/input/harness/fereg_f0_scc.py" verify-input --run-root "$RUN_ROOT"
python3 -I -B -S "$RUN_ROOT/input/harness/fereg_f0_scc.py" setup --run-root "$RUN_ROOT"
