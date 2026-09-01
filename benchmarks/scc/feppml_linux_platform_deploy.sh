#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "usage: $0 LOCAL_PREPARED_RUN_ROOT" >&2
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

python3 "$HARNESS" verify-input --run-root "$LOCAL_RUN_ROOT"
LOCAL_PREPARATION_SHA=$(shasum -a 256 "$LOCAL_RUN_ROOT/input/preparation.json" | awk '{print $1}')
case "$LOCAL_PREPARATION_SHA" in
    ''|*[!0-9a-f]*) echo "invalid local preparation digest" >&2; exit 2 ;;
esac
ssh scc "test ! -e '$REMOTE_RUN_ROOT'"
ssh scc "mkdir -p '$REMOTE_RUN_ROOT'"
rsync -av --partial --progress "$LOCAL_RUN_ROOT/" "scc:$REMOTE_RUN_ROOT/"
ssh scc "source /etc/bashrc && module --force purge && module load python3/3.12.4 && python3 '$REMOTE_RUN_ROOT/input/harness/benchmarks/scc/feppml_linux_platform_scc.py' verify-input --run-root '$REMOTE_RUN_ROOT'"
ssh scc "source /etc/bashrc && module --force purge && module load python3/3.12.4 && python3 '$REMOTE_RUN_ROOT/input/harness/benchmarks/scc/feppml_linux_platform_scc.py' record-deployment --run-root '$REMOTE_RUN_ROOT' --preparation-sha256 '$LOCAL_PREPARATION_SHA'"
rsync -av --partial --progress \
    "scc:$REMOTE_RUN_ROOT/receipts/deployment.json" \
    "$LOCAL_RUN_ROOT/receipts/deployment.json"
python3 "$HARNESS" verify-deployment --run-root "$LOCAL_RUN_ROOT"
printf 'FEPPML_LINUX_PLATFORM_DEPLOY_COMPLETE run_root=%s\n' "$REMOTE_RUN_ROOT"
