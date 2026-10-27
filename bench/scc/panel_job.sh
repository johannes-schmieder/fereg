#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_veneto
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${MODE:?}" "${UPSTREAM_JOB:?}"
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2 julia/1.11.7
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 OMP_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export JULIA_DEPOT_PATH="$RUN_ROOT/environment/julia-depot" JULIA_PKG_OFFLINE=true
export JULIA_CPU_TARGET='generic;sandybridge,-xsaveopt,clone_all;haswell,-rdrnd,base(1)'
export PYTHONDONTWRITEBYTECODE=1 MPLCONFIGDIR="$RUN_ROOT/environment/matplotlib"
export R_LIBS_USER="$RUN_ROOT/environment/r-library"
cd "$RUN_ROOT/source"
python3 - "$RUN_ROOT" "$UPSTREAM_JOB" "$MODE" <<'PY'
import json,pathlib,subprocess,sys,time
from bench.campaign_collect import accounting_records
r=pathlib.Path(sys.argv[1]);job=sys.argv[2]
for attempt in range(12):
    raw=subprocess.run(['qacct','-j',job],capture_output=True,text=True)
    if raw.returncode==0:
        records=accounting_records(raw.stdout)
        assert len(records)==1 and records[0]['jobnumber']==job
        assert records[0]['failed']=='0' and records[0]['exit_status']=='0',records
        break
    if attempt==11:raise RuntimeError('prerequisite accounting unavailable')
    time.sleep(5)
assert (r/'environment/COMPLETE').read_text().strip()=='PANEL_SETUP_COMPLETE'
if sys.argv[3]=='run':
    assert (r/'smoke/COMPLETE').read_text().strip()=='PANEL_TASK_COMPLETE'
    smoke=json.loads((r/'smoke/results.json').read_text())
    assert smoke['native_gate_pass'] and smoke['complete'] and len(smoke['records'])==smoke['expected_fits']
    assert all(s['passed'] for s in smoke['science'] if s['kind']=='dense-oracle' and s['arm']=='fereg')
PY
task=${SGE_TASK_ID:-1}
if [[ "$task" == undefined ]]; then task=1; fi
python3 bench/panel_campaign.py "$MODE" "$RUN_ROOT" --task "$task"
printf 'PANEL_JOB_COMPLETE job=%s task=%s\n' "$JOB_ID" "$task"
