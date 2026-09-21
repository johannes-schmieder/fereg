#!/bin/bash -l
#$ -P welfgr
#$ -N veneto_ppml_fix
#$ -pe omp 14
#$ -l mem_per_core=3G
#$ -l h_rt=00:22:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${BENCHMARK_ROOT:?}"
module purge
module load stata-mp/19
module load python3/3.12.4
export PYTHONDONTWRITEBYTECODE=1
cd "$RUN_ROOT"
sha256sum -c plugin.sha256
python3 "$RUN_ROOT/verify_ppml.py" "$RUN_ROOT" "$BENCHMARK_ROOT"
