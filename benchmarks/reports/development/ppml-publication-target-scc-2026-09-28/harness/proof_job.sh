#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_target_proof
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-publication-target-proof02 && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 R/4.5.2 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
for tag in positive zero; do
 "$reference/env/python/bin/python" "$RUN_ROOT/$tag/run.py" --run-root "$RUN_ROOT/$tag" \
  --source /projectnb/welfgr/fereg/dev/20260928-publication-target-01/source \
  --baseline /projectnb/welfgr/fereg/dev/20260928-coefficient-stop-01/source \
  --reference "$reference" --scratch "$TMPDIR/proof-$tag"
done
python3 - "$RUN_ROOT" <<'CHECK'
from pathlib import Path
import json,sys
r=Path(sys.argv[1])
for tag in ('positive','zero'):
 d=json.loads((r/tag/'results/complete.json').read_text())
 assert d['complete'] and d['oracle_pass'] and d['candidate_science'] and d['baseline_science']
CHECK
printf 'FEREG_PUBLICATION_TARGET_PROOFS_SUCCESS job=%s\n' "$JOB_ID"
