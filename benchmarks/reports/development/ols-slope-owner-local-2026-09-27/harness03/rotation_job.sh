#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_ols_slope_pairs
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=01:00:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${SMOKE_JOB:?}" "${BASELINE_SOURCE:?}" "${REFERENCE_RUN:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/* && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
# A scheduler dependency establishes termination, not success.
for attempt in 1 2 3 4 5 6; do
    if qacct -j "$SMOKE_JOB" > "$RUN_ROOT/smoke03-accounting.txt" 2> "$RUN_ROOT/smoke03-accounting-error.txt"; then
        break
    fi
    sleep 10
done
python3 - "$RUN_ROOT/smoke03-accounting.txt" <<'PY'
from pathlib import Path
import sys
fields = dict(line.split(None, 1) for line in Path(sys.argv[1]).read_text().splitlines() if line.split() and not line.startswith("="))
assert fields["failed"].strip() == fields["exit_status"].strip() == "0", fields
PY
"$REFERENCE_RUN/env/python/bin/python" "$RUN_ROOT/harness03/slope_rotation.py" \
    --run "$RUN_ROOT" --plan "$RUN_ROOT/registration03.json" \
    --baseline "$BASELINE_SOURCE" --candidate "$RUN_ROOT/source" --reference "$REFERENCE_RUN" \
    --output "$RUN_ROOT/rotation03-results"
printf 'FEREG_OLS_SLOPE_ROTATIONS_SUCCESS job=%s\n' "$JOB_ID"
