#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "usage: $0 LOCAL_DEPLOYED_RUN_ROOT" >&2
    exit 2
fi
LOCAL_RUN_ROOT=$(cd "$1" && pwd)
RUN_ID=$(basename "$LOCAL_RUN_ROOT")
if [[ ! "$RUN_ID" =~ ^[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "invalid feppml Linux platform run ID" >&2
    exit 2
fi
REMOTE_RUN_ROOT=/projectnb/welfgr/fereg/runs/$RUN_ID
HARNESS="$LOCAL_RUN_ROOT/input/harness/benchmarks/scc/feppml_linux_platform_scc.py"

python3 -I -B -S "$HARNESS" verify-deployment --run-root "$LOCAL_RUN_ROOT"
QSUB_COMMAND=$(python3 -I -B -S "$HARNESS" print-submission-command \
    --run-root "$LOCAL_RUN_ROOT")
RAW_JOB=$(ssh scc "source /etc/bashrc && module --force purge && module load python3/3.12.4 && python3 -I -B -S '$REMOTE_RUN_ROOT/input/harness/benchmarks/scc/feppml_linux_platform_scc.py' verify-deployment --run-root '$REMOTE_RUN_ROOT' >/dev/null && cd '$REMOTE_RUN_ROOT' && $QSUB_COMMAND")
JOB_ID=${RAW_JOB%%.*}
case "$JOB_ID" in
    ''|*[!0-9]*) echo "invalid qsub response" >&2; exit 1 ;;
esac
python3 -I -B -S "$HARNESS" record-submission \
    --run-root "$LOCAL_RUN_ROOT" --job-id "$JOB_ID"
rsync -av --partial --progress \
    "$LOCAL_RUN_ROOT/receipts/submission.json" \
    "scc:$REMOTE_RUN_ROOT/receipts/submission.json"
printf 'FEPPML_LINUX_PLATFORM_SUBMITTED job=%s\n' "$JOB_ID"
