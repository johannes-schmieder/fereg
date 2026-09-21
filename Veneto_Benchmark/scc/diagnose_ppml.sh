#!/bin/bash -l
#$ -P welfgr
#$ -N veneto_ppml_trace
#$ -pe omp 14
#$ -l cpu_type=E5-2680v4
#$ -l mem_per_core=3G
#$ -l h_rt=00:30:00
#$ -j y
set -euo pipefail
umask 077
: "${BENCHMARK_ROOT:?}" "${DIAGNOSTIC_ROOT:?}"
module purge
module load stata-mp/19
module load python3/3.12.4
export PYTHONDONTWRITEBYTECODE=1
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export OMP_NUM_THREADS=14 RAYON_NUM_THREADS=14
python3 "$DIAGNOSTIC_ROOT/diagnose_ppml.py" "$BENCHMARK_ROOT" "$DIAGNOSTIC_ROOT"
