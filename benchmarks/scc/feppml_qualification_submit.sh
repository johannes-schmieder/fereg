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
HARNESS="$RUN_ROOT/input/harness/benchmarks/scc/feppml_qualification_scc.py"
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/runs/[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "invalid exact run root" >&2
    exit 2
fi
CONTROLLER_PYTHON=/share/pkg.8/python3/3.12.4/install/bin/python3
QSUB=/usr/local/bin/qsub
QSTAT=/usr/local/bin/qstat
QACCT=/usr/local/bin/qacct
QRLS=/usr/local/bin/qrls
for value in "$CONTROLLER_PYTHON" "$QSUB" "$QSTAT" "$QACCT" "$QRLS"; do
    case "$value" in /*) ;; *) echo "scheduler/controller executable is not absolute" >&2; exit 2 ;; esac
done
test "$($CONTROLLER_PYTHON -I -B -S --version 2>&1)" = "Python 3.12.4"
CONTROLLER_ROOT="$RUN_ROOT/work/submission-controller"
test ! -e "$CONTROLLER_ROOT"
/usr/bin/mkdir -p "$CONTROLLER_ROOT/home" "$CONTROLLER_ROOT/tmp"
/usr/bin/env -i \
    HOME="$CONTROLLER_ROOT/home" TMPDIR="$CONTROLLER_ROOT/tmp" \
    PATH="$(dirname "$CONTROLLER_PYTHON"):$(dirname "$QSUB"):/usr/bin:/bin" \
    LANG=C LC_ALL=C TZ=UTC USER=johannes LOGNAME=johannes RUN_ROOT="$RUN_ROOT" \
    SGE_ROOT=/usr/local/sge/sge_root SGE_CELL=default \
    FEPPML_QSUB_EXECUTABLE="$QSUB" FEPPML_QSTAT_EXECUTABLE="$QSTAT" \
    FEPPML_QACCT_EXECUTABLE="$QACCT" FEPPML_QRLS_EXECUTABLE="$QRLS" \
    FEPPML_CONTROLLER_PYTHON="$CONTROLLER_PYTHON" \
    "$CONTROLLER_PYTHON" -I -B -S "$HARNESS" record-submission \
    --run-root "$RUN_ROOT"
printf 'FEPPML_QUALIFICATION_SUBMITTED run_root=%s\n' "$RUN_ROOT"
