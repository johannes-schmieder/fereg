#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_veneto
#$ -pe omp 28
#$ -l num_proc=28
#$ -l cpu_type=E5-2680v4
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${NSLOTS:?}"
[[ "$RUN_ROOT" == /* && "$NSLOTS" == 28 ]]
module purge
module load stata-mp/19
module load python3/3.12.4
module load R/4.5.2
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export OMP_NUM_THREADS=28 RAYON_NUM_THREADS=28
export PYTHONDONTWRITEBYTECODE=1
export R_LIBS_USER="$RUN_ROOT/env/R"
cd "$RUN_ROOT"
if [[ "${SMOKE:-0}" == 1 ]]; then
    "$RUN_ROOT/env/python/bin/python" "$RUN_ROOT/controller/run_scc.py" "$RUN_ROOT" --smoke
else
    "$RUN_ROOT/env/python/bin/python" "$RUN_ROOT/controller/run_scc.py" "$RUN_ROOT" --task "$SGE_TASK_ID"
fi
