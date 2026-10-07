#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_bench
#$ -pe omp 28
#$ -l num_proc=28
#$ -l mem_per_core=3G
#$ -l h_rt=04:00:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${MODE:?}"
test "$NSLOTS" = 28
test -f "$RUN_ROOT/environment/COMPLETE"
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export R_LIBS_USER="$RUN_ROOT/environment/r-library"
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 OMP_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
cd "$RUN_ROOT/source"
python3 - "$RUN_ROOT" "$MODE" <<'PY'
import hashlib,json,pathlib,sys
r=pathlib.Path(sys.argv[1])
for name,expected in json.loads((r/'environment/runtime.json').read_text()).items():
    assert hashlib.sha256((r/name).read_bytes()).hexdigest()==expected,name
if sys.argv[2]=='run':
    smoke=json.loads((r/'smoke/results.json').read_text())
    assert smoke['native_gate_pass'] and (r/'smoke/COMPLETE').exists()
PY
task=${SGE_TASK_ID:-1}
if [[ "$task" == undefined ]]; then task=1; fi
python3 bench/campaign.py "$MODE" "$RUN_ROOT" --task "$task"
printf 'CAMPAIGN_JOB_COMPLETE job=%s task=%s\n' "$JOB_ID" "$task"
