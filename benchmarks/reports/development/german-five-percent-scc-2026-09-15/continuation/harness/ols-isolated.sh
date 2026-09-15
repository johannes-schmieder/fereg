#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_german5_ols_isolated
#$ -pe omp 16
#$ -l h_rt=03:00:00
#$ -l mem_per_core=3G
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
python3 "$RUN_ROOT/harness/run-ols-isolated.py" --root "$RUN_ROOT" --work-name comparison-ols-isolated-1 --native-memory-bytes 42949672960
export OMP_NUM_THREADS=1 RAYON_NUM_THREADS=1
/usr/bin/time -v python3 "$RUN_ROOT/harness/analyze-selected.py" --root "$RUN_ROOT" --family ols --work-name comparison-ols-isolated-1 --report-name analysis-ols-isolated-1
