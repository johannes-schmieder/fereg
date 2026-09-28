#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_public_host_03
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-public-host-rotation03 && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import json,sys,hashlib
r=Path(sys.argv[1]);build=Path('/projectnb/welfgr/fereg/dev/20260928-public-host-01')
for f,digest in json.loads((r/'deployment.json').read_text()).items():assert hashlib.sha256((r/f).read_bytes()).hexdigest()==digest,f
for name in ['mac-validation01.json','linux-validation01.json']:
 d=json.loads((r/name).read_text());assert d['status']=='success' and d['tests_failed']==0 and d['public_break_success'] and d['native_break_success']
d=json.loads((build/'output/summary.json').read_text());assert d['status']=='success' and d['tests_failed']==0 and d['tests_passed']==6
assert 'FEREG_PUBLIC_HOST_VALIDATION_SUCCESS' in (build/'job.log').read_text()
for mode in ['native','public']:assert 'PASS: END_FIXTURE|estimate_ppml_eclass_break|rc=0' in (build/'output'/('break-'+mode+'.log')).read_text()
assert 'BOUNDARY: feppml public scope' in (build/'output/break-public.log').read_text()
assert 'Ran 24 tests' in (build/'output/static.log').read_text() and '\nOK' in (build/'output/static.log').read_text()
(r/'diagnostic02').mkdir()
(r/'diagnostic02/complete.json').write_text(json.dumps(dict(all_fits_passed=True,linux_validation_job='7763686',mac_break=True,linux_break=True))+'\n')
GATE
"$reference/env/python/bin/python" "$RUN_ROOT/rotation03-harness/performance/launch_rotation.py" \
 --run-root "$RUN_ROOT" --attempt rotation03 \
 --baseline /projectnb/welfgr/fereg/dev/20260928-host-retained-07/source \
 --candidate /projectnb/welfgr/fereg/dev/20260928-public-host-01/source \
 --reference "$reference" --scratch "$TMPDIR/host-processors"
"$reference/env/python/bin/python" "$RUN_ROOT/rotation03-harness/performance/summarize_rotations.py" "$RUN_ROOT/rotation03-results" --output "$RUN_ROOT/summary03"
printf 'FEREG_PUBLIC_HOST_TIMING_SUCCESS job=%s\n' "$JOB_ID"
