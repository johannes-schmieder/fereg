#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_performance_pairs
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=03:00:00
#$ -t 1
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${ROTATION:?}" "${BASELINE_SOURCE:?}" "${CANDIDATE_SOURCE:?}" "${REFERENCE_RUN:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/* && "$NSLOTS" == 28 ]]
[[ "$ROTATION" =~ ^rotation[0-9]{2,}$ ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
"$REFERENCE_RUN/env/python/bin/python" "$RUN_ROOT/$ROTATION-harness/performance/launch_rotation.py" \
    --run-root "$RUN_ROOT" --attempt "$ROTATION" \
    --baseline "$BASELINE_SOURCE" --candidate "$CANDIDATE_SOURCE" \
    --reference "$REFERENCE_RUN" --scratch "$TMPDIR/performance-rotations"
printf 'FEREG_SCHWARZ_ROTATIONS_SUCCESS job=%s task=%s\n' "$JOB_ID" "$SGE_TASK_ID"
