#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_tasks
#$ -pe omp28 28
#$ -l num_proc=28
#$ -l mem_per_core=3G
#$ -l h_rt=12:00:00
#$ -j y
# Run several frozen manifest tasks back to back in one whole-node allocation,
# for example a predeclared validation of failed production blocks. Each task
# keeps its own budget, host observation and results directory; the scheduler
# identity remains the allocation's own. Mirrors job.sh's preflight, except that
# it requires only the upstream setup job.
# Usage: qsub -v RUN_ROOT=...,UPSTREAM_JOB=SETUP_JOB,TASKS="194 182" campaign_tasks.sh
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${UPSTREAM_JOB:?}" "${TASKS:?}"
if [[ -f "$RUN_ROOT/input/policy.json" ]]; then
    export CAMPAIGN_POLICY_OVERRIDE
    CAMPAIGN_POLICY_OVERRIDE=$(cat "$RUN_ROOT/input/policy.json")
fi
test "$NSLOTS" = "${EXPECTED_SLOTS:-28}"
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
python3 - "$RUN_ROOT" <<'PY'
import hashlib,json,pathlib,sys
r=pathlib.Path(sys.argv[1])
for name,expected in json.loads((r/'environment/runtime.json').read_text()).items():
    assert hashlib.sha256((r/name).read_bytes()).hexdigest()==expected,name
PY
failed=0
for task in $TASKS; do
    # A failed block stays visible in its own log; the next block still runs.
    rc=0
    python3 bench/campaign.py run "$RUN_ROOT" --task "$task" \
        --frozen-inputs "$RUN_ROOT/input/frozen-inputs.json" --observe-host \
        > "$RUN_ROOT/logs/task-$task.log" 2>&1 || rc=$?
    printf 'CAMPAIGN_TASK_RESULT job=%s task=%s rc=%s\n' "$JOB_ID" "$task" "$rc"
    [[ "$rc" == 0 ]] || failed=1
done
test "$failed" = 0
printf 'CAMPAIGN_TASKS_COMPLETE job=%s tasks=%s\n' "$JOB_ID" "$TASKS"
