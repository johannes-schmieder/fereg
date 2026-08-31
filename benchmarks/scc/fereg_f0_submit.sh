#!/usr/bin/env bash
set -eo pipefail
source /etc/bashrc
set -u
module --force purge
module load python3/3.12.4

if [ "$#" -ne 1 ]; then
    echo "usage: $0 /projectnb/welfgr/fereg/runs/RUN_ID" >&2
    exit 2
fi
RUN_ROOT=$1
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/runs/[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "RUN_ROOT is not an exact fereg run directory" >&2
    exit 2
fi

python3 "$RUN_ROOT/input/harness/fereg_f0_scc.py" verify-input --run-root "$RUN_ROOT"
test ! -e "$RUN_ROOT/receipts/submission.json"
mkdir -p "$RUN_ROOT/logs"
TASK_COUNT=$(awk 'NR > 1 { count += 1 } END { print count + 0 }' "$RUN_ROOT/input/tasks.tsv")
case "$TASK_COUNT" in
    ''|*[!0-9]*) echo "invalid task count" >&2; exit 2 ;;
esac
if [ "$TASK_COUNT" -lt 2 ]; then
    echo "task inventory cannot support the smoke-plus-array schedule" >&2
    exit 2
fi

SETUP_RAW=$(qsub -terse -P welfgr -o "$RUN_ROOT/logs" -v RUN_ROOT="$RUN_ROOT" "$RUN_ROOT/input/harness/fereg_f0_setup_job.sh")
SETUP_JOB=${SETUP_RAW%%.*}
case "$SETUP_JOB" in ''|*[!0-9]*) echo "invalid setup qsub response: $SETUP_RAW" >&2; exit 1 ;; esac

SMOKE_RAW=$(qsub -terse -P welfgr -o "$RUN_ROOT/logs" -hold_jid "$SETUP_JOB" -t 1 -v RUN_ROOT="$RUN_ROOT" "$RUN_ROOT/input/harness/fereg_f0_job.sh")
SMOKE_JOB=${SMOKE_RAW%%.*}
case "$SMOKE_JOB" in ''|*[!0-9]*) echo "invalid smoke qsub response: $SMOKE_RAW" >&2; exit 1 ;; esac

ARRAY_RAW=$(qsub -terse -P welfgr -o "$RUN_ROOT/logs" -hold_jid "$SMOKE_JOB" -t "2-$TASK_COUNT" -v RUN_ROOT="$RUN_ROOT" "$RUN_ROOT/input/harness/fereg_f0_job.sh")
ARRAY_JOB=${ARRAY_RAW%%.*}
case "$ARRAY_JOB" in ''|*[!0-9]*) echo "invalid array qsub response: $ARRAY_RAW" >&2; exit 1 ;; esac

COLLECTOR_RAW=$(qsub -terse -P welfgr -o "$RUN_ROOT/logs" -hold_jid "$ARRAY_JOB" -v RUN_ROOT="$RUN_ROOT" "$RUN_ROOT/input/harness/fereg_f0_collector_job.sh")
COLLECTOR_JOB=${COLLECTOR_RAW%%.*}
case "$COLLECTOR_JOB" in ''|*[!0-9]*) echo "invalid collector qsub response: $COLLECTOR_RAW" >&2; exit 1 ;; esac

python3 "$RUN_ROOT/input/harness/fereg_f0_scc.py" record-submission \
    --run-root "$RUN_ROOT" \
    --setup-job "$SETUP_JOB" \
    --smoke-job "$SMOKE_JOB" \
    --array-job "$ARRAY_JOB" \
    --collector-job "$COLLECTOR_JOB"
printf 'FEREG_F0_SUBMISSION_SUCCESS setup=%s smoke=%s array=%s collector=%s\n' "$SETUP_JOB" "$SMOKE_JOB" "$ARRAY_JOB" "$COLLECTOR_JOB"
