#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_slope_numa
#$ -cwd
#$ -j y
set -euo pipefail
module load python3/3.12.4 stata-mp/19
export STATA_MP_PATH="$(command -v stata-mp)"
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
cpus=$(python3 harness/select-cpus.py)
exec taskset -c "$cpus" python3 harness/check.py
