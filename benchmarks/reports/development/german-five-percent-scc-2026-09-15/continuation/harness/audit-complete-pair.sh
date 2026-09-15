#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_german5_pair_audit
#$ -pe omp 4
#$ -l h_rt=01:30:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a
set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?}"
module --force purge
module load python3/3.12.4
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
python3 "$RUN_ROOT/harness/analyze-selected.py" --root "$RUN_ROOT" --family ppml --work-name launcher-smoke-v2 --report-name analysis-selector-smoke --completed-first-pair
/usr/bin/time -v python3 "$RUN_ROOT/harness/analyze-selected.py" --root "$RUN_ROOT" --family ppml --work-name comparison-ppml --report-name analysis-complete-ppml-pair --completed-first-pair
