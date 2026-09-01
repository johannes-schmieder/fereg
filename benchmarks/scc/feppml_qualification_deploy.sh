#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "usage: $0 LOCAL_PREPARED_RUN_ROOT" >&2
    exit 2
fi
LOCAL_RUN_ROOT=$1
RUN_ID=$(basename "$LOCAL_RUN_ROOT")
if [[ ! "$RUN_ID" =~ ^[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "invalid feppml qualification run ID" >&2
    exit 2
fi
REMOTE_RUN_ROOT=/projectnb/welfgr/fereg/runs/$RUN_ID
HARNESS="$LOCAL_RUN_ROOT/input/harness/benchmarks/scc/feppml_qualification_scc.py"
LOCAL_PYTHON=/opt/anaconda3/bin/python3.12
LOCAL_PYTHON_SHA256=b4f9a14acf8ad0f7a9775cd1ff39d1bad69e632dd93a841b9ed6d7357b7840a7

test -f "$LOCAL_RUN_ROOT/input/preparation.json"
test -f "$LOCAL_RUN_ROOT/input/PREPARED"
test -x "$LOCAL_PYTHON"
test "$("$LOCAL_PYTHON" -I -B -S --version 2>&1)" = "Python 3.12.2"
test "$(/usr/bin/shasum -a 256 "$LOCAL_PYTHON" | /usr/bin/awk '{print $1}')" = "$LOCAL_PYTHON_SHA256"
"$LOCAL_PYTHON" -I -B -S "$HARNESS" verify-input --run-root "$LOCAL_RUN_ROOT"
LOCAL_PREPARATION_SHA=$(/usr/bin/shasum -a 256 "$LOCAL_RUN_ROOT/input/preparation.json" | /usr/bin/awk '{print $1}')
case "$LOCAL_PREPARATION_SHA" in ''|*[!0-9a-f]*) echo "invalid local preparation digest" >&2; exit 2 ;; esac
ssh scc "test ! -e '$REMOTE_RUN_ROOT'"
ssh scc "mkdir -p '$REMOTE_RUN_ROOT'"
rsync -av --partial --progress "$LOCAL_RUN_ROOT/" "scc:$REMOTE_RUN_ROOT/"
ssh scc "source /etc/bashrc && module --force purge && module load python3/3.12.4 && CONTROLLER_PYTHON=/share/pkg.8/python3/3.12.4/install/bin/python3 && test \"\$(\"\$CONTROLLER_PYTHON\" -I -B -S --version 2>&1)\" = 'Python 3.12.4' && CONTROLLER_ROOT='$REMOTE_RUN_ROOT/work/controller/deployment' && test ! -e \"\$CONTROLLER_ROOT\" && /usr/bin/mkdir -p \"\$CONTROLLER_ROOT/home\" \"\$CONTROLLER_ROOT/tmp\" && exec /usr/bin/env -i HOME=\"\$CONTROLLER_ROOT/home\" TMPDIR=\"\$CONTROLLER_ROOT/tmp\" PATH=/share/pkg.8/python3/3.12.4/install/bin:/usr/bin:/bin LANG=C LC_ALL=C TZ=UTC RUN_ROOT='$REMOTE_RUN_ROOT' FEPPML_CONTROLLER_PYTHON=\"\$CONTROLLER_PYTHON\" \"\$CONTROLLER_PYTHON\" -I -B -S '$REMOTE_RUN_ROOT/input/harness/benchmarks/scc/feppml_qualification_scc.py' record-deployment --run-root '$REMOTE_RUN_ROOT' --preparation-sha256 '$LOCAL_PREPARATION_SHA'"
printf 'FEPPML_QUALIFICATION_DEPLOY_COMPLETE run_root=%s\n' "$REMOTE_RUN_ROOT"
