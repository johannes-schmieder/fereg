#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_german_memory
#$ -cwd
#$ -j y
set -euo pipefail
module load python3/3.12.4 stata-mp/19
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
python3 run.py
