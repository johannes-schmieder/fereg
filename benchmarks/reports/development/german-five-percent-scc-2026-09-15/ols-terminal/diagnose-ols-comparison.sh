#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_german5_ols_diagnosis
#$ -pe omp 4
#$ -l h_rt=00:15:00
#$ -l mem_per_core=3G
#$ -j y
#$ -m a
set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?}"
module --force purge
module load python3/3.12.4
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 RAYON_NUM_THREADS=1
/usr/bin/time -v python3 "$RUN_ROOT/harness/diagnose-ols-comparison.py" --root "$RUN_ROOT"
