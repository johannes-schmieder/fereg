#!/bin/bash -l
#$ -P welfgr
#$ -N feppml_q_cell
#$ -pe omp 16
#$ -l h_rt=11:45:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?RUN_ROOT is required}"
: "${SGE_TASK_ID:?SGE_TASK_ID is required}"
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/runs/[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "invalid exact run root" >&2
    exit 2
fi
case "$SGE_TASK_ID" in ''|*[!0-9]*) echo "invalid task ID" >&2; exit 2 ;; esac
module --force purge
module load stata-mp/19
module load python3/3.12.4
export OMP_NUM_THREADS="${NSLOTS:-1}"
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1
test -f "$RUN_ROOT/receipts/SETUP.SUCCESS"
CONTROLLER_PYTHON=/share/pkg.8/python3/3.12.4/install/bin/python3
STATA_EXECUTABLE=$(command -v stata-mp)
case "$CONTROLLER_PYTHON" in /*/python3) ;; *) echo "invalid Python executable" >&2; exit 2 ;; esac
case "$STATA_EXECUTABLE" in /*/stata-mp) ;; *) echo "invalid Stata MP executable" >&2; exit 2 ;; esac
test "$($CONTROLLER_PYTHON -I -B -S --version 2>&1)" = "Python 3.12.4"
CONTROLLER_ROOT="$RUN_ROOT/work/controller/task-$SGE_TASK_ID"
test ! -e "$CONTROLLER_ROOT"
/usr/bin/mkdir -p "$CONTROLLER_ROOT/home" "$CONTROLLER_ROOT/tmp"
exec /usr/bin/env -i \
    HOME="$CONTROLLER_ROOT/home" TMPDIR="$CONTROLLER_ROOT/tmp" \
    PATH="$(dirname "$CONTROLLER_PYTHON"):$(dirname "$STATA_EXECUTABLE"):/usr/bin:/bin" \
    LANG=C LC_ALL=C TZ=UTC RUN_ROOT="$RUN_ROOT" \
    JOB_ID="${JOB_ID:?JOB_ID is required}" NSLOTS="${NSLOTS:?NSLOTS is required}" \
    SGE_TASK_ID="$SGE_TASK_ID" \
    FEPPML_CONTROLLER_PYTHON="$CONTROLLER_PYTHON" \
    "$CONTROLLER_PYTHON" -I -B -S \
    "$RUN_ROOT/input/harness/benchmarks/scc/feppml_qualification_scc.py" task \
    --run-root "$RUN_ROOT" --task-id "$SGE_TASK_ID"
