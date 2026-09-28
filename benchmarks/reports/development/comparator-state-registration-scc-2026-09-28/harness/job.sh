#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_comparator
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-comparator-state-01 && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=4
export MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 RAYON_NUM_THREADS=4 NUMBA_NUM_THREADS=4
export OMP_MAX_ACTIVE_LEVELS=1 OMP_NESTED=FALSE
export NUMBA_CACHE_DIR="$TMPDIR/numba" MPLCONFIGDIR="$TMPDIR/mpl"
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
"$reference/env/python/bin/python" "$RUN_ROOT/run.py" --run-root "$RUN_ROOT" \
    --reference "$reference" --scratch "$TMPDIR/comparator-state"
printf 'FEREG_COMPARATOR_STATE_SUCCESS job=%s\n' "$JOB_ID"
