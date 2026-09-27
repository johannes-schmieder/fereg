#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_performance_diagnose
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=02:00:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${SOURCE_ROOT:?}" "${REFERENCE_RUN:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/* && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
mkdir -p "$TMPDIR/performance-inputs"
"$REFERENCE_RUN/env/python/bin/python" "$RUN_ROOT/harness/performance/diagnose.py" \
    --source "$SOURCE_ROOT" --refs "$REFERENCE_RUN/references" \
    --source-manifest "$RUN_ROOT/source-manifest.json" \
    --manifest "$REFERENCE_RUN/input/manifest.json" --plan "$RUN_ROOT/plan.json" \
    --output "$RUN_ROOT/diagnostic01" --scratch "$TMPDIR/performance-inputs"
printf 'FEREG_PERFORMANCE_DIAGNOSTICS_SUCCESS job=%s\n' "$JOB_ID"
