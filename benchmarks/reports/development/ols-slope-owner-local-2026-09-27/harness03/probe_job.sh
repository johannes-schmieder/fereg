#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_slope_probe
#$ -pe omp 8
#$ -l mem_per_core=4G,h_rt=00:10:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${BASELINE_SOURCE:?}" "${REFERENCE_RUN:?}"
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
python3 - "$RUN_ROOT" <<'PY'
from pathlib import Path
import hashlib,json,sys
root=Path(sys.argv[1])
for name in ('summary.json','slope-oracle.json'):
 result=json.loads((root/'output'/name).read_text())
 assert result['status']=='success' and result['tests_failed']==0 and result['completed']
smoke=json.loads((root/'output/summary.json').read_text())
assert hashlib.sha256((root/'source/dist/fereg_linux.plugin').read_bytes()).hexdigest()==smoke['plugin_sha256']
PY
"$REFERENCE_RUN/env/python/bin/python" "$RUN_ROOT/harness03/slope_rotation.py" \
    --run "$RUN_ROOT" --plan "$RUN_ROOT/registration03.json" \
    --baseline "$BASELINE_SOURCE" --candidate "$RUN_ROOT/source" --reference "$REFERENCE_RUN" \
    --output "$RUN_ROOT/smoke-probe03" --smoke
printf 'FEREG_OLS_SLOPE_PROBE_SUCCESS job=%s\n' "$JOB_ID"
