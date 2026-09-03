#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_solver_v2
#$ -pe omp 28
#$ -l h_rt=02:00:00
#$ -l mem_per_core=4G
#$ -l m_core=28
#$ -l cpu_arch=broadwell
#$ -l cpu_type=E5-2680v4
#$ -t 1-16
#$ -j y
#$ -m a

source /etc/bashrc
set -euo pipefail

: "${RUN_ROOT:?RUN_ROOT is required}"
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]; then
    echo "RUN_ROOT is not an exact fereg development run directory" >&2
    exit 2
fi
if [[ "${NSLOTS:-}" != "28" || ! "${SGE_TASK_ID:-}" =~ ^([1-9]|1[0-6])$ ]]; then
    echo "solver campaign requires task 1-16 and exactly 28 slots" >&2
    exit 2
fi

module --force purge
module load stata-mp/19
module load python3/3.12.4

export OMP_NUM_THREADS=1
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1

task=$(printf '%02d' "$SGE_TASK_ID")
qhost_receipt="$RUN_ROOT/work/qhost-task-$task.txt"
test ! -e "$qhost_receipt"
qhost -h "$HOSTNAME" -F m_core,cpu_arch,cpu_type > "$qhost_receipt"
chmod a-w "$qhost_receipt"

candidate=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["candidate_root"])' "$RUN_ROOT/input/config.json")
python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" run-task \
    --run-root "$RUN_ROOT" --qhost "$qhost_receipt"

printf 'FEPPML_SOLVER_DEV_TASK_SUCCESS task=%s job=%s\n' "$SGE_TASK_ID" "${JOB_ID:-unknown}"
