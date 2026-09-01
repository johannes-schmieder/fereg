#!/bin/bash -l
#$ -P welfgr
#$ -N feppml_q_collect
#$ -pe omp 1
#$ -l h_rt=00:30:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?RUN_ROOT is required}"
module --force purge
module load python3/3.12.4
CONTROLLER_PYTHON=/share/pkg.8/python3/3.12.4/install/bin/python3
case "$CONTROLLER_PYTHON" in /*/python3) ;; *) echo "invalid Python executable" >&2; exit 2 ;; esac
test "$($CONTROLLER_PYTHON -I -B -S --version 2>&1)" = "Python 3.12.4"
CONTROLLER_ROOT="$RUN_ROOT/work/controller/collector"
test ! -e "$CONTROLLER_ROOT"
/usr/bin/mkdir -p "$CONTROLLER_ROOT/home" "$CONTROLLER_ROOT/tmp"
exec /usr/bin/env -i \
    HOME="$CONTROLLER_ROOT/home" TMPDIR="$CONTROLLER_ROOT/tmp" \
    PATH="$(dirname "$CONTROLLER_PYTHON"):/usr/bin:/bin" \
    LANG=C LC_ALL=C TZ=UTC RUN_ROOT="$RUN_ROOT" \
    JOB_ID="${JOB_ID:?JOB_ID is required}" NSLOTS="${NSLOTS:?NSLOTS is required}" \
    FEPPML_CONTROLLER_PYTHON="$CONTROLLER_PYTHON" \
    "$CONTROLLER_PYTHON" -I -B -S \
    "$RUN_ROOT/input/harness/benchmarks/scc/feppml_qualification_scc.py" \
    collect --run-root "$RUN_ROOT"
