#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "usage: $0 LOCAL_PREPARED_RUN_ROOT" >&2
    exit 2
fi
LOCAL_RUN_ROOT=$1
RUN_ID=$(basename "$LOCAL_RUN_ROOT")
if [[ ! "$RUN_ID" =~ ^[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "invalid run ID" >&2
    exit 2
fi
REMOTE_RUN_ROOT=/projectnb/welfgr/fereg/runs/$RUN_ID

test -f "$LOCAL_RUN_ROOT/input/preparation.json"
test -f "$LOCAL_RUN_ROOT/input/PREPARED"
python3 -I -B -S "$LOCAL_RUN_ROOT/input/harness/fereg_f0_scc.py" verify-input --run-root "$LOCAL_RUN_ROOT"

# A run ID is immutable. Deployment is incremental and never deletes or
# overlays a previous attempt.
ssh scc "test ! -e '$REMOTE_RUN_ROOT'"
ssh scc "mkdir -p '$REMOTE_RUN_ROOT'"
rsync -av --partial --progress "$LOCAL_RUN_ROOT/" "scc:$REMOTE_RUN_ROOT/"
ssh scc "source /etc/bashrc && module --force purge && module load python3/3.12.4 && python3 -I -B -S '$REMOTE_RUN_ROOT/input/harness/fereg_f0_scc.py' verify-input --run-root '$REMOTE_RUN_ROOT'"
printf 'FEREG_F0_DEPLOY_SUCCESS run_root=%s\n' "$REMOTE_RUN_ROOT"
