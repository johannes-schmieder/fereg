#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_light
#$ -pe omp 28
#$ -l num_proc=28
#$ -l cpu_type=E5-2680v4
#$ -l mem_per_core=3G
#$ -l h_rt=12:00:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}"
export LIGHT_TASK_STARTED=$(date +%s)
LIGHT_TASK_ID=${LIGHT_TASK_ID:-${SGE_TASK_ID:-1}}
[[ "$LIGHT_TASK_ID" == undefined ]] && LIGHT_TASK_ID=1
export LIGHT_TASK_ID
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/runs/* && "$NSLOTS" == 28 ]]
module purge
module load stata-mp/19 python3/3.12.4 R/4.5.2
export R_LIBS_USER="$RUN_ROOT/env/R"
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export OMP_NUM_THREADS=28 RAYON_NUM_THREADS=28 NUMBA_NUM_THREADS=28
export PYTHONDONTWRITEBYTECODE=1
controller="$RUN_ROOT/harness/benchmarks/light"
if [[ "$LIGHT_TASK_ID" == 1 && ! -e "$RUN_ROOT/runtime.json" ]]; then
  if [[ ! -e "$RUN_ROOT/prebuilt.json" ]]; then
    export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo
    export RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
    export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
    export PATH="$CARGO_HOME/bin:$PATH"
    export CARGO_TARGET_DIR="$RUN_ROOT/build"
    export FEREG_GIT_COMMIT=e23f15cad16c2f0b2368326aea579921a069ca9a
    export FEREG_BUILD_DATE=2026-09-19
    export CARGO_NET_OFFLINE=true
    cd "$RUN_ROOT/candidate"
    bash scripts/build_linux.sh > "$RUN_ROOT/logs/build.log" 2>&1
  fi
    "$RUN_ROOT/env/python/bin/python" "$controller/provenance.py" freeze "$RUN_ROOT"
fi
cd "$RUN_ROOT"
"$RUN_ROOT/env/python/bin/python" "$controller/runner.py" "$RUN_ROOT" "$LIGHT_TASK_ID"
