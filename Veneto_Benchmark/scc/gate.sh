#!/bin/bash -l
#$ -P welfgr
#$ -N veneto_accept
#$ -j y
set -euo pipefail
umask 077
trap 'code=$?; if [[ "$code" != 0 ]]; then exit 100; fi' EXIT
module purge
module load python3/3.12.4
export OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1
python3 "$RUN_ROOT/controller/accept_smoke.py" "$RUN_ROOT" "$SMOKE_JOB_ID"
