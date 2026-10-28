#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_bench
#$ -pe mpi_28_tasks_per_node_u 28
#$ -l num_proc=28
#$ -l mem_per_core=3G
#$ -l h_rt=12:00:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${MODE:?}" "${UPSTREAM_JOB:?}"
test "$NSLOTS" = 28
test -f "$RUN_ROOT/environment/COMPLETE"
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2 julia/1.11.7
export R_LIBS_USER="$RUN_ROOT/environment/r-library"
export JULIA_DEPOT_PATH="$RUN_ROOT/environment/julia-depot" JULIA_PKG_OFFLINE=true
export JULIA_CPU_TARGET='generic;sandybridge,-xsaveopt,clone_all;haswell,-rdrnd,base(1)'
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 OMP_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
cd "$RUN_ROOT/source"
python3 - "$UPSTREAM_JOB" <<'PY'
import subprocess,sys,time
from bench.campaign_collect import accounting_records
for attempt in range(12):
    result=subprocess.run(['qacct','-j',sys.argv[1]],capture_output=True,text=True)
    if result.returncode==0:
        records=accounting_records(result.stdout)
        assert len(records)==1 and records[0]['jobnumber']==sys.argv[1]
        assert records[0]['failed']=='0' and records[0]['exit_status']=='0',records
        break
    if attempt==11:raise RuntimeError('upstream accounting unavailable')
    time.sleep(5)
PY
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
python3 bench/campaign.py "$MODE" "$RUN_ROOT" --task "$task" \
    --frozen-inputs "$RUN_ROOT/input/frozen-inputs.json" --observe-host
printf 'CAMPAIGN_JOB_COMPLETE job=%s task=%s\n' "$JOB_ID" "$task"
