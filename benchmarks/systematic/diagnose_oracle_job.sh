#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_oracle_trace
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${CONTROLLER:?}"
: "${FEREG_SCC_RUNS_ROOT:?Set the allowed SCC run-root parent}"
[[ "$FEREG_SCC_RUNS_ROOT" == /* && "$FEREG_SCC_RUNS_ROOT" != / ]]
[[ "$RUN_ROOT" == "${FEREG_SCC_RUNS_ROOT%/}"/* && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 OMP_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
"$RUN_ROOT/env/python/bin/python" "$CONTROLLER/diagnose_oracle.py"
