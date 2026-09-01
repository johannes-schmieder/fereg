#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 LOCAL_RUN_ROOT CANDIDATE_REPO HARNESS_REPO" >&2
    exit 2
fi
LOCAL_RUN_ROOT=$(cd "$1" && pwd)
CANDIDATE_REPO=$(cd "$2" && pwd)
HARNESS_REPO=$(cd "$3" && pwd)
RUN_ID=$(basename "$LOCAL_RUN_ROOT")
if [[ ! "$RUN_ID" =~ ^[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "invalid feppml Linux platform run ID" >&2
    exit 2
fi
REMOTE_RUN_ROOT=/projectnb/welfgr/fereg/runs/$RUN_ID
HARNESS="$LOCAL_RUN_ROOT/input/harness/benchmarks/scc/feppml_linux_platform_scc.py"

python3 "$HARNESS" verify-submission --run-root "$LOCAL_RUN_ROOT"
JOB_ID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["payload"]["job_id"])' \
    "$LOCAL_RUN_ROOT/receipts/submission.json")
case "$JOB_ID" in
    ''|*[!0-9]*) echo "invalid recorded job ID" >&2; exit 2 ;;
esac
if ssh scc "qstat -j '$JOB_ID' >/dev/null 2>&1"; then
    echo "SCC job is still active" >&2
    exit 3
fi

ssh scc "source /etc/bashrc && module --force purge && module load python3/3.12.4 && python3 '$REMOTE_RUN_ROOT/input/harness/benchmarks/scc/feppml_linux_platform_scc.py' verify-job --run-root '$REMOTE_RUN_ROOT'"
for target in \
    "$LOCAL_RUN_ROOT/output/fereg_linux.plugin" \
    "$LOCAL_RUN_ROOT/output/capture.json" \
    "$LOCAL_RUN_ROOT/output/native-attestation.json" \
    "$LOCAL_RUN_ROOT/output/native-attestation.json.status" \
    "$LOCAL_RUN_ROOT/output/native-attestation.json.stata.log" \
    "$LOCAL_RUN_ROOT/output/native-attestation.json.wrapper.do" \
    "$LOCAL_RUN_ROOT/output/native-attestation.json.invocation.json" \
    "$LOCAL_RUN_ROOT/logs/build.log" \
    "$LOCAL_RUN_ROOT/receipts/job.json" \
    "$LOCAL_RUN_ROOT/JOB.SUCCESS" \
    "$LOCAL_RUN_ROOT/receipts/qacct.txt"; do
    test ! -e "$target"
done
rsync -av --partial --progress \
    "scc:$REMOTE_RUN_ROOT/output/fereg_linux.plugin" \
    "$LOCAL_RUN_ROOT/output/fereg_linux.plugin"
rsync -av --partial --progress \
    "scc:$REMOTE_RUN_ROOT/output/capture.json" \
    "$LOCAL_RUN_ROOT/output/capture.json"
for evidence in \
    native-attestation.json \
    native-attestation.json.status \
    native-attestation.json.stata.log \
    native-attestation.json.wrapper.do \
    native-attestation.json.invocation.json; do
    rsync -av --partial --progress \
        "scc:$REMOTE_RUN_ROOT/output/$evidence" \
        "$LOCAL_RUN_ROOT/output/$evidence"
done
rsync -av --partial --progress \
    "scc:$REMOTE_RUN_ROOT/logs/build.log" \
    "$LOCAL_RUN_ROOT/logs/build.log"
rsync -av --partial --progress \
    "scc:$REMOTE_RUN_ROOT/receipts/job.json" \
    "$LOCAL_RUN_ROOT/receipts/job.json"
rsync -av --partial --progress \
    "scc:$REMOTE_RUN_ROOT/JOB.SUCCESS" \
    "$LOCAL_RUN_ROOT/JOB.SUCCESS"

QACCT_TMP=$(mktemp /private/tmp/feppml-linux-qacct.XXXXXX)
trap 'rm -f "$QACCT_TMP"' EXIT
QACCT_READY=no
for attempt in $(seq 1 30); do
    if ssh scc "qacct -j '$JOB_ID'" >"$QACCT_TMP" 2>/dev/null && \
            test -s "$QACCT_TMP"; then
        QACCT_READY=yes
        break
    fi
    if [ "$attempt" -lt 30 ]; then
        sleep 10
    fi
done
if [ "$QACCT_READY" != yes ]; then
    echo "SCC qacct remained unavailable after the bounded retry window" >&2
    exit 4
fi
mv "$QACCT_TMP" "$LOCAL_RUN_ROOT/receipts/qacct.txt"
chmod 444 "$LOCAL_RUN_ROOT/receipts/qacct.txt"
trap - EXIT
python3 "$HARNESS" close --run-root "$LOCAL_RUN_ROOT" \
    --candidate-repo "$CANDIDATE_REPO" \
    --harness-repo "$HARNESS_REPO"
printf 'FEPPML_LINUX_PLATFORM_COLLECT_COMPLETE run_root=%s\n' "$LOCAL_RUN_ROOT"
