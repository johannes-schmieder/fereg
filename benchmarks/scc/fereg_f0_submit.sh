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

python3 -I -B -S "$RUN_ROOT/input/harness/fereg_f0_scc.py" verify-input --run-root "$RUN_ROOT"
test ! -e "$RUN_ROOT/receipts/submission.json"
test ! -e "$RUN_ROOT/receipts/qsub"
mkdir -p "$RUN_ROOT/logs"
mkdir -p "$RUN_ROOT/work/scheduler-home"
mkdir "$RUN_ROOT/receipts/qsub"
cd "$RUN_ROOT"
date -u '+%Y-%m-%dT%H:%M:%SZ' > "$RUN_ROOT/receipts/qsub/started-at-utc.txt"
TASK_COUNT=$(awk 'NR > 1 { count += 1 } END { print count + 0 }' "$RUN_ROOT/input/tasks.tsv")
case "$TASK_COUNT" in
    ''|*[!0-9]*) echo "invalid task count" >&2; exit 2 ;;
esac
if [ "$TASK_COUNT" -lt 1 ]; then
    echo "task inventory is empty" >&2
    exit 2
fi
RUN_ID=${RUN_ROOT##*/}
RUN_TAG=$(printf '%s' "$RUN_ID" | /usr/bin/sha256sum | /usr/bin/cut -c1-12)
SETUP_NAME="f0s_$RUN_TAG"
ARRAY_NAME="f0a_$RUN_TAG"
COLLECTOR_NAME="f0c_$RUN_TAG"

SCHEDULER_ENV=(/usr/bin/env -i HOME="$RUN_ROOT/work/scheduler-home" PATH=/usr/local/bin:/usr/bin:/bin LANG=C LC_ALL=C TZ=UTC SGE_ROOT=/usr/local/sge/sge_root)
QSUB=/usr/local/bin/qsub
QSTAT=/usr/local/bin/qstat
test "$(/usr/bin/sha256sum "$QSUB" | /usr/bin/cut -d ' ' -f1)" = fdd46c981d8f3107a44e6985ca9e689e4969b7c6281c8e56a78420c084fd7a6e
test "$(/usr/bin/sha256sum "$QSTAT" | /usr/bin/cut -d ' ' -f1)" = 0fb1c3334e9885f7df3873986c71465ef8b5528674a938441dbaefb6214c9941

"${SCHEDULER_ENV[@]}" "$QSUB" -clear -terse -P welfgr -S /bin/bash -j y -m a -N "$SETUP_NAME" -wd "$RUN_ROOT" -o "$RUN_ROOT/logs" -pe omp 16 -l h_rt=04:00:00 -l mem_per_core=4G -v RUN_ROOT="$RUN_ROOT" "$RUN_ROOT/input/harness/fereg_f0_setup_job.sh" > "$RUN_ROOT/receipts/qsub/setup.txt"
SETUP_RAW=$(tr -d '\n' < "$RUN_ROOT/receipts/qsub/setup.txt")
SETUP_JOB=${SETUP_RAW%%.*}
case "$SETUP_JOB" in ''|*[!0-9]*) echo "invalid setup qsub response: $SETUP_RAW" >&2; exit 1 ;; esac
"${SCHEDULER_ENV[@]}" "$QSTAT" -j "$SETUP_JOB" > "$RUN_ROOT/receipts/qsub/setup.qstat.txt"

"${SCHEDULER_ENV[@]}" "$QSUB" -clear -terse -P welfgr -S /bin/bash -j y -m a -N "$ARRAY_NAME" -wd "$RUN_ROOT" -o "$RUN_ROOT/logs" -pe omp 16 -l h_rt=11:45:00 -l mem_per_core=4G -hold_jid "$SETUP_JOB" -t "1-$TASK_COUNT" -v RUN_ROOT="$RUN_ROOT" "$RUN_ROOT/input/harness/fereg_f0_job.sh" > "$RUN_ROOT/receipts/qsub/array.txt"
ARRAY_RAW=$(tr -d '\n' < "$RUN_ROOT/receipts/qsub/array.txt")
ARRAY_JOB=${ARRAY_RAW%%.*}
case "$ARRAY_JOB" in ''|*[!0-9]*) echo "invalid array qsub response: $ARRAY_RAW" >&2; exit 1 ;; esac
"${SCHEDULER_ENV[@]}" "$QSTAT" -j "$ARRAY_JOB" > "$RUN_ROOT/receipts/qsub/array.qstat.txt"

"${SCHEDULER_ENV[@]}" "$QSUB" -clear -terse -P welfgr -S /bin/bash -j y -m a -N "$COLLECTOR_NAME" -wd "$RUN_ROOT" -o "$RUN_ROOT/logs" -l h_rt=00:30:00 -l mem_per_core=4G -hold_jid "$ARRAY_JOB" -v RUN_ROOT="$RUN_ROOT" "$RUN_ROOT/input/harness/fereg_f0_collector_job.sh" > "$RUN_ROOT/receipts/qsub/collector.txt"
COLLECTOR_RAW=$(tr -d '\n' < "$RUN_ROOT/receipts/qsub/collector.txt")
COLLECTOR_JOB=${COLLECTOR_RAW%%.*}
case "$COLLECTOR_JOB" in ''|*[!0-9]*) echo "invalid collector qsub response: $COLLECTOR_RAW" >&2; exit 1 ;; esac
"${SCHEDULER_ENV[@]}" "$QSTAT" -j "$COLLECTOR_JOB" > "$RUN_ROOT/receipts/qsub/collector.qstat.txt"

date -u '+%Y-%m-%dT%H:%M:%SZ' > "$RUN_ROOT/receipts/qsub/completed-at-utc.txt"
chmod 0444 "$RUN_ROOT"/receipts/qsub/*.txt
python3 -I -B -S "$RUN_ROOT/input/harness/fereg_f0_scc.py" record-submission \
    --run-root "$RUN_ROOT"
printf 'FEREG_F0_SUBMISSION_SUCCESS setup=%s array=%s collector=%s\n' "$SETUP_JOB" "$ARRAY_JOB" "$COLLECTOR_JOB"
