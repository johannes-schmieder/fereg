#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_systematic
#$ -pe omp 28
#$ -l num_proc=28
#$ -l cpu_type=E5-2680v4
#$ -l h_rt=12:00:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${PLAN:?}"
CONTROLLER=${CONTROLLER:-$RUN_ROOT/controller}
: "${FEREG_SCC_RUNS_ROOT:?Set the allowed SCC run-root parent}"
[[ "$FEREG_SCC_RUNS_ROOT" == /* && "$FEREG_SCC_RUNS_ROOT" != / ]]
[[ "$RUN_ROOT" == "${FEREG_SCC_RUNS_ROOT%/}"/* && "$NSLOTS" == 28 ]]
test -f "$RUN_ROOT/env/SETUP.SUCCESS"
module purge
module load stata-mp/19
module load python3/3.12.4
module load R/4.5.2
export R_LIBS_USER="$RUN_ROOT/env/R"
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export OMP_NUM_THREADS=28
export NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
task_id=${SGE_TASK_ID:-1}
if [[ "$task_id" == undefined ]]; then task_id=1; fi
[[ "$task_id" =~ ^[1-9][0-9]*$ ]]
"$RUN_ROOT/env/python/bin/python" "$CONTROLLER/runner.py" "$RUN_ROOT" "$PLAN" "$task_id"
