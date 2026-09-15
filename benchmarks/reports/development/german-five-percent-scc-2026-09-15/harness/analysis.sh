#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_german5_audit
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
args=()
if [ "${SMOKE:-0}" = 1 ]; then
  args+=(--smoke)
else
  python3 -c 'import json,os; from pathlib import Path; p=Path(os.environ["RUN_ROOT"])/"analysis-smoke/summary.json"; assert json.loads(p.read_text())["passed"]'
fi
/usr/bin/time -v python3 "$RUN_ROOT/harness/analyze.py" --root "$RUN_ROOT" "${args[@]}"
