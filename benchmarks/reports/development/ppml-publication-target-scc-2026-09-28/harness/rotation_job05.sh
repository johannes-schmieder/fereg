#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_target_05
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-publication-target-rotation05 && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import json,sys,hashlib
r=Path(sys.argv[1]);build=Path('/projectnb/welfgr/fereg/dev/20260928-publication-target-01');proof=build.with_name('20260928-publication-target-proof02b')
for f,digest in json.loads((r/'deployment.json').read_text()).items():assert hashlib.sha256((r/f).read_bytes()).hexdigest()==digest,f
for base,names in [(build/'output',['summary.json','oracles.json']),(r,['mac-validation.json'])]:
 for name in names:
  d=json.loads((base/name).read_text());assert d['status']=='success' and d['tests_failed']==0
for tag in ('positive','zero'):
 d=json.loads((proof/tag/'results/complete.json').read_text());assert d['complete'] and d['oracle_pass'] and d['candidate_science'] and d['baseline_science']
assert 'FEREG_PUBLICATION_TARGET_VALIDATION_SUCCESS' in (build/'job.log').read_text()
assert 'FEREG_PUBLICATION_TARGET_PROOFS_SUCCESS' in (proof/'job.log').read_text()
(r/'diagnostic02').mkdir()
(r/'diagnostic02/complete.json').write_text(json.dumps(dict(all_fits_passed=True,linux_job='7763463',proofs='original-positive-and-zero'))+'\n')
GATE
"$reference/env/python/bin/python" "$RUN_ROOT/rotation05-harness/performance/launch_rotation.py" \
 --run-root "$RUN_ROOT" --attempt rotation05 \
 --baseline /projectnb/welfgr/fereg/dev/20260928-coefficient-stop-01/source \
 --candidate /projectnb/welfgr/fereg/dev/20260928-publication-target-01/source \
 --reference "$reference" --scratch "$TMPDIR/publication-target"
printf 'FEREG_PUBLICATION_TARGET_05ATION_SUCCESS job=%s\n' "$JOB_ID"
