#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_ols_budget
#$ -pe omp 4
#$ -l mem_per_core=6G,mem_total=480G,h_rt=00:15:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-ols-timeout-memory-01 && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=4
export MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 RAYON_NUM_THREADS=4
export OMP_MAX_ACTIVE_LEVELS=1 OMP_NESTED=FALSE
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
"$reference/env/python/bin/python" -B -m unittest discover -s "$RUN_ROOT/tests/python" -p test_bounded_diagnostic.py -v
"$reference/env/python/bin/python" "$RUN_ROOT/run.py" --run-root "$RUN_ROOT" --scratch "$TMPDIR/ols-budget"
printf 'FEREG_OLS_BUDGET_DIAGNOSTIC_COMPLETE job=%s\n' "$JOB_ID"
