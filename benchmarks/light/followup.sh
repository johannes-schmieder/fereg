#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_followup
#$ -pe omp 28
#$ -l num_proc=28
#$ -l cpu_type=E5-2680v4
#$ -l mem_per_core=3G
#$ -l h_rt=12:00:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}"
: "${PARENT_ROOT:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/runs/* && "$NSLOTS" == 28 ]]
export LIGHT_TASK_STARTED=$(date +%s)
module purge
module load stata-mp/19 python3/3.12.4 R/4.5.2
export R_LIBS_USER="$RUN_ROOT/env/R"
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export OMP_NUM_THREADS=28 RAYON_NUM_THREADS=28 NUMBA_NUM_THREADS=28
export PYTHONDONTWRITEBYTECODE=1
controller="$RUN_ROOT/harness/benchmarks/light"
test ! -e "$RUN_ROOT/runtime.json"
"$RUN_ROOT/env/python/bin/python" "$controller/followup_plan.py" "$RUN_ROOT" "$PARENT_ROOT"
"$RUN_ROOT/env/python/bin/python" "$controller/provenance.py" freeze "$RUN_ROOT"
cd "$RUN_ROOT"
"$RUN_ROOT/env/python/bin/python" "$controller/followup.py" "$RUN_ROOT"
