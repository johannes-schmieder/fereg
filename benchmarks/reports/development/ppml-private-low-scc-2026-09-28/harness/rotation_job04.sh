#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_private_04
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-private-low-rotation04 && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import json,sys,hashlib
r=Path(sys.argv[1]);build=Path('/projectnb/welfgr/fereg/dev/20260928-private-low-02')
for f,digest in json.loads((r/'deployment.json').read_text()).items():assert hashlib.sha256((r/f).read_bytes()).hexdigest()==digest,f
for name in ['mac-validation02.json','linux-validation02.json']:
 d=json.loads((r/name).read_text());assert d['status']=='success' and d['tests_failed']==0 and d['rust_distinct_passed']>=104
for name,count in [('summary.json',1),('oracles.json',6)]:
 d=json.loads((build/'output'/name).read_text());assert d['status']=='success' and d['tests_failed']==0 and d['tests_passed']==count
assert 'FEREG_PRIVATE_LOW_VALIDATION_SUCCESS' in (build/'job.log').read_text()
(r/'diagnostic02').mkdir();(r/'diagnostic02/complete.json').write_text(json.dumps(dict(all_fits_passed=True,linux_validation_job='7764784'))+'\n')
GATE
"$reference/env/python/bin/python" "$RUN_ROOT/rotation04-harness/performance/launch_rotation.py" \
 --run-root "$RUN_ROOT" --attempt rotation04 \
 --baseline /projectnb/welfgr/fereg/dev/20260928-private-moments-01/source \
 --candidate /projectnb/welfgr/fereg/dev/20260928-private-low-02/source \
 --reference "$reference" --scratch "$TMPDIR/private-moments"
"$reference/env/python/bin/python" "$RUN_ROOT/rotation04-harness/performance/summarize_rotations.py" "$RUN_ROOT/rotation04-results" --output "$RUN_ROOT/summary04"
mkdir "$RUN_ROOT/scaling/diagnostic02"
cp "$RUN_ROOT/diagnostic02/complete.json" "$RUN_ROOT/scaling/diagnostic02/"
"$reference/env/python/bin/python" "$RUN_ROOT/scaling/rotation05-harness/performance/launch_rotation.py" \
 --run-root "$RUN_ROOT/scaling" --attempt rotation05 \
 --baseline /projectnb/welfgr/fereg/dev/20260928-private-low-02/source \
 --candidate /projectnb/welfgr/fereg/dev/20260928-private-low-02/source \
 --reference "$reference" --scratch "$TMPDIR/private-scaling"
"$reference/env/python/bin/python" "$RUN_ROOT/scaling/rotation05-harness/performance/summarize_rotations.py" "$RUN_ROOT/scaling/rotation05-results" --output "$RUN_ROOT/scaling/summary05"
printf 'FEREG_PRIVATE_LOW_TIMING_SUCCESS job=%s\n' "$JOB_ID"
