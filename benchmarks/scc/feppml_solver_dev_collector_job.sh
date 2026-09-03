#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_solver_merge
#$ -pe omp 1
#$ -l h_rt=00:30:00
#$ -l mem_per_core=2G
#$ -j y
#$ -m a

source /etc/bashrc
set -euo pipefail

: "${RUN_ROOT:?RUN_ROOT is required}"
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]; then
    echo "RUN_ROOT is not an exact fereg development run directory" >&2
    exit 2
fi
if [[ "${NSLOTS:-}" != "1" ]]; then
    echo "collector requires exactly one slot" >&2
    exit 2
fi

module --force purge
module load python3/3.12.4
candidate=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["candidate_root"])' "$RUN_ROOT/input/config.json")
python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" merge --run-root "$RUN_ROOT"
printf 'FEPPML_SOLVER_DEV_MERGE_SUCCESS job=%s\n' "${JOB_ID:-unknown}"
