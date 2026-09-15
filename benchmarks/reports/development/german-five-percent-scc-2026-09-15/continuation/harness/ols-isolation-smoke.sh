#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_german5_ols_smoke
#$ -pe omp 4
#$ -l h_rt=00:30:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a
set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?}"
module --force purge
module load stata-mp/19
module load python3/3.12.4
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 RAYON_NUM_THREADS=4
python3 "$RUN_ROOT/harness/run-ols-isolated.py" --root "$RUN_ROOT" --work-name ols-isolation-smoke --smoke --native-memory-bytes 10737418240
export OMP_NUM_THREADS=1 RAYON_NUM_THREADS=1
python3 "$RUN_ROOT/harness/analyze-selected.py" --root "$RUN_ROOT" --family ols --work-name ols-isolation-smoke --report-name analysis-ols-isolation-smoke
