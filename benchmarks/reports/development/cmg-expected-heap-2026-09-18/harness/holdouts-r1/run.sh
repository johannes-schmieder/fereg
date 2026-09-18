#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_mem_calibration
#$ -pe omp 28
#$ -l h_rt=00:20:00
#$ -l mem_per_core=3G
#$ -l num_proc=28
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${SOURCE_ROOT:?}"
[[ "$RUN_ROOT" == ${DEV_ROOT}/cmg-heap-holdouts-20260918-r1 ]]
[[ "$SOURCE_ROOT" == ${DEV_ROOT}/cmg-heap-20260918-r2/candidate/source ]]
test -f "$SOURCE_ROOT/../../DONE"
export BASELINE_ROOT=${DEV_ROOT}/cmg-heap-20260918-r2/baseline/source
test -f "$BASELINE_ROOT/../../DONE"
export BASELINE_MANIFEST_SHA256="$(sha256sum "$BASELINE_ROOT/../source.sha256" | cut -d' ' -f1)"
(cd "$BASELINE_ROOT/.." && sha256sum -c source.sha256) > "$RUN_ROOT/logs/baseline-before.txt"
cd "$RUN_ROOT"
sha256sum -c harness.sha256 > logs/harness-before.txt
(cd "$SOURCE_ROOT/.." && sha256sum -c source.sha256) > logs/source-before.txt
module load python3/3.12.4 stata-mp/19
export STATA_MP_PATH="$(command -v stata-mp)"
export SOURCE_ROOT SOURCE_MANIFEST_SHA256="$(sha256sum "$SOURCE_ROOT/../source.sha256" | cut -d' ' -f1)"
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 OMP_NUM_THREADS=8
cpus=$(python3 harness/select-cpus.py)
python3 - <<'CHECK'
import json
from pathlib import Path
p=Path('${DEV_ROOT}/cmg-heap-calibration-20260918-r1')
s=json.loads((p/'summary.json').read_text())
assert s['native_accuracy_pass'] and s['performance_pass']
assert not any(x['timing_flags'] for x in json.loads((p/'phase-summary.json').read_text()))
CHECK
python3 harness/generate.py
taskset -c "$cpus" python3 harness/run.py > logs/results.txt 2>&1
python3 harness/analyze_pairs.py "$RUN_ROOT" > logs/phases.txt
sha256sum -c harness.sha256 > logs/harness-after.txt
printf 'PRAGMATIC_CALIBRATION_PASS job=%s\n' "$JOB_ID" > DONE
