#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_ppml_proof
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-original-ppml-positive-01 && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 R/4.5.2 stata-mp/19
export PYTHONPATH="$RUN_ROOT/tests/python" PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
"$reference/env/python/bin/python" "$RUN_ROOT/run.py" --run-root "$RUN_ROOT" \
 --source /projectnb/welfgr/fereg/dev/20260928-joint-operator-01/source \
 --reference "$reference" --scratch "$TMPDIR/ppml-positive"
printf 'FEREG_PPML_ORIGINAL_POSITIVE_SUCCESS job=%s\n' "$JOB_ID"
