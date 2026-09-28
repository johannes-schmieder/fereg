#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_center_03
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-centered-inline-rotation03 && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import json,sys,hashlib,re
r=Path(sys.argv[1]);build=Path('/projectnb/welfgr/fereg/dev/20260928-centered-inline-02')
for f,digest in json.loads((r/'deployment.json').read_text()).items():assert hashlib.sha256((r/f).read_bytes()).hexdigest()==digest,f
for base,names in [(build/'output',['summary.json','oracles.json']),(r,['mac-validation.json'])]:
 for name in names:
  d=json.loads((base/name).read_text());assert d['status']=='success' and d['tests_failed']==0
for f,n in [('irls-tests.log',61),('centered-tests.log',3),('profile-tests.log',3)]:
 text=(build/'output'/f).read_text();assert len(re.findall(r'^test .+ \.\.\. ok$',text,re.M))==n,f
assert 'FEREG_CENTERED_INLINE_VALIDATION_SUCCESS' in (build/'job.log').read_text()
GATE
# Correct the zero-match linear-session filter before any timed application.
# Invoke the existing test binary directly: no build or changed source/artifact.
python3 - "$RUN_ROOT" <<'SUPPLEMENT'
from pathlib import Path
import os,subprocess,sys,re
r=Path(sys.argv[1]);target=Path('/projectnb/welfgr/fereg/dev/20260928-centered-inline-02/work/target/debug/deps')
binaries=[p for p in target.glob('fereg_core-*') if p.is_file() and p.suffix=='' and os.access(p,os.X_OK)]
assert len(binaries)==1,binaries
with (r/'linear-session-tests.log').open('w') as out:
 subprocess.run([str(binaries[0]),'ols::linear_session','--test-threads=4'],stdout=out,stderr=subprocess.STDOUT,check=True,timeout=120)
assert len(re.findall(r'^test .+ \.\.\. ok$',(r/'linear-session-tests.log').read_text(),re.M))==7
(r/'diagnostic02').mkdir()
(r/'diagnostic02/complete.json').write_text('{"all_fits_passed": true, "linux_validation_job": "7763539", "linear_tests": 7}\n')
SUPPLEMENT
"$reference/env/python/bin/python" "$RUN_ROOT/rotation03-harness/performance/launch_rotation.py" \
 --run-root "$RUN_ROOT" --attempt rotation03 \
 --baseline /projectnb/welfgr/fereg/dev/20260928-publication-target-01/source \
 --candidate /projectnb/welfgr/fereg/dev/20260928-centered-inline-02/source \
 --reference "$reference" --scratch "$TMPDIR/centered-inline"
"$reference/env/python/bin/python" "$RUN_ROOT/rotation03-harness/performance/summarize_rotations.py" "$RUN_ROOT/rotation03-results" --output "$RUN_ROOT/summary03"
printf 'FEREG_CENTERED_INLINE_TIMING_SUCCESS job=%s\n' "$JOB_ID"
