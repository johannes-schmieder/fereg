#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_thread_cost
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:15:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-thread-cost-02 && "$NSLOTS" == 28 ]]
module purge
module load stata-mp/19 python3/3.12.4
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
"$reference/env/python/bin/python" "$RUN_ROOT/profile.py"
printf 'FEREG_THREAD_COST_SUCCESS job=%s\n' "$JOB_ID"
