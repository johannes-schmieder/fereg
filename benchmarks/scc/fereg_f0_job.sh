#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_f0_pair
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
    echo "RUN_ROOT is not an exact fereg run directory" >&2
    exit 2
fi
case "$SGE_TASK_ID" in
    ''|*[!0-9]*) echo "SGE_TASK_ID is not numeric" >&2; exit 2 ;;
esac

module --force purge
module load stata-mp/19
module load python3/3.12.4

# One Stata process is active at a time. fereg receives its registered native
# thread count per cell; nested numeric libraries remain serial.
export OMP_NUM_THREADS="${NSLOTS:-1}"
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1

if [ "$SGE_TASK_ID" -gt 1 ]; then
    test -f "$RUN_ROOT/receipts/tasks/task-001.SUCCESS"
fi

python3 "$RUN_ROOT/input/harness/fereg_f0_scc.py" task \
    --run-root "$RUN_ROOT" \
    --task-id "$SGE_TASK_ID"
