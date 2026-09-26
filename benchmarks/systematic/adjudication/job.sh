#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_slope_adjudication
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=05:00:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${ADJUDICATION_SOURCE:?}" "${ADJUDICATION_OUTPUT:?}" "${FEREG_SCC_RUNS_ROOT:?}"
[[ "$RUN_ROOT" == "${FEREG_SCC_RUNS_ROOT%/}"/* && "$NSLOTS" == 28 ]]
[[ "$ADJUDICATION_SOURCE" == "$RUN_ROOT"/diagnostics/* && "$ADJUDICATION_OUTPUT" == "$RUN_ROOT"/diagnostics/* ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export OPENBLAS_NUM_THREADS=4 OMP_NUM_THREADS=4 MKL_NUM_THREADS=4 NUMEXPR_NUM_THREADS=1 PYTHONDONTWRITEBYTECODE=1
"$RUN_ROOT/env/python/bin/python" "$ADJUDICATION_SOURCE/run.py" "$RUN_ROOT" "$ADJUDICATION_OUTPUT"
