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
    echo "invalid exact run root" >&2
    exit 2
fi
CONTROLLER_PYTHON=/share/pkg.8/python3/3.12.4/install/bin/python3
test "$($CONTROLLER_PYTHON -I -B -S --version 2>&1)" = "Python 3.12.4"
CONTROLLER_ROOT="$RUN_ROOT/work/controller/close"
test ! -e "$CONTROLLER_ROOT"
/usr/bin/mkdir -p "$CONTROLLER_ROOT/home" "$CONTROLLER_ROOT/tmp"
exec /usr/bin/env -i \
    HOME="$CONTROLLER_ROOT/home" TMPDIR="$CONTROLLER_ROOT/tmp" \
    PATH="$(dirname "$CONTROLLER_PYTHON"):/usr/bin:/bin" \
    LANG=C LC_ALL=C TZ=UTC RUN_ROOT="$RUN_ROOT" \
    FEPPML_CONTROLLER_PYTHON="$CONTROLLER_PYTHON" \
    "$CONTROLLER_PYTHON" -I -B -S \
    "$RUN_ROOT/input/harness/benchmarks/scc/feppml_qualification_scc.py" \
    close --run-root "$RUN_ROOT"
