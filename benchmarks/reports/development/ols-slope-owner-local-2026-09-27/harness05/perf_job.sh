#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_slope_perf
#$ -pe omp 8
#$ -l mem_per_core=3G,h_rt=00:10:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${CONTROL_JOB:?}"
module purge
module load python3/3.12.4 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1
for attempt in 1 2 3 4 5 6; do
    if qacct -j "$CONTROL_JOB" > "$RUN_ROOT/control04-accounting.txt"; then break; fi
    sleep 10
done
python3 - "$RUN_ROOT" <<'PY2'
from pathlib import Path
import sys,json
root=Path(sys.argv[1])
fields=dict(line.split(None,1) for line in (root/'control04-accounting.txt').read_text().splitlines() if line.split() and not line.startswith('='))
assert fields['failed'].strip()==fields['exit_status'].strip()=='0',fields
for phase in ('diagnostic04-results','control04-results'):
    assert json.loads((root/phase/'complete.json').read_text())['complete']
PY2
python3 "$RUN_ROOT/harness05/slope_perf.py" --diagnostics "$RUN_ROOT/diagnostic04-results" --output "$RUN_ROOT/perf05-results"
printf 'FEREG_SLOPE_PERF_SUCCESS job=%s\n' "$JOB_ID"
