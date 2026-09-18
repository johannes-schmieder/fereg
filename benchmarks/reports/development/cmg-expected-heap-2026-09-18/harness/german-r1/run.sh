#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_german_heap
#$ -pe omp 28
#$ -l num_proc=28
#$ -l mem_per_core=3G
#$ -l h_rt=00:20:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${SOURCE_ROOT:?}"
[[ "$RUN_ROOT" == ${DEV_ROOT}/cmg-heap-german-20260918-r1 ]]
[[ "$SOURCE_ROOT" == ${DEV_ROOT}/cmg-heap-20260918-r2/candidate/source ]]
cd "$RUN_ROOT"
module load python3/3.12.4 stata-mp/19
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 PYTHONDONTWRITEBYTECODE=1
sha256sum -c harness.sha256 > logs/harness-before.txt
(cd "$SOURCE_ROOT/.." && sha256sum -c source.sha256) > logs/source-before.txt
python3 - <<'PY'
import json
from pathlib import Path
p=Path('${DEV_ROOT}/cmg-heap-holdouts-20260918-r1')
assert (p/'DONE').exists()
assert json.loads((p/'summary.json').read_text())['native_accuracy_pass']
pairs=json.loads((p/'pairs.json').read_text())
assert len(pairs)==4 and all(x['science']=='pass' for x in pairs)
PY
cpus=$(python3 select-cpus.py)
taskset -c "$cpus" python3 run.py > logs/results.txt 2>&1
(cd "$SOURCE_ROOT/.." && sha256sum -c source.sha256) > logs/source-after.txt
sha256sum -c harness.sha256 > logs/harness-after.txt
printf 'GERMAN_EXPECTED_HEAP_OBSERVATION_COMPLETE job=%s\n' "$JOB_ID" > DONE
