#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_auto_policy
#$ -pe omp 1
#$ -l h_rt=00:30:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a
source /etc/bashrc
set -euo pipefail
: "${RUN_ROOT:?RUN_ROOT is required}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
[[ "${NSLOTS:-}" == 1 ]]
module --force purge
module load python3/3.12.4
controller=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["controller"]["root"])' "$RUN_ROOT/input/config.json")
python3 -I -B -S "$controller/scripts/ppml_auto_calibration.py" fit-scc --run-root "$RUN_ROOT"
printf 'FEPPML_AUTO_POLICY_PASS job=%s\n' "$JOB_ID"
