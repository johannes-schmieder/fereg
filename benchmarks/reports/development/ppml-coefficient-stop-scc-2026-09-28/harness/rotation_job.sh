#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_stop_fresh
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-coefficient-stop-rotation03 && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
candidate=/projectnb/welfgr/fereg/dev/20260928-coefficient-stop-01
baseline=/projectnb/welfgr/fereg/dev/20260928-joint-operator-01/source
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
qacct -j 7763383 > "$RUN_ROOT/build-accounting.txt"
python3 - "$RUN_ROOT" "$candidate" <<'PY'
from pathlib import Path
import json,sys
r,c=map(Path,sys.argv[1:])
f=dict(l.split(None,1) for l in (r/'build-accounting.txt').read_text().splitlines() if l.split() and not l.startswith('='))
assert f['failed'].strip()==f['exit_status'].strip()=='0'
for base,names in [(c/'output',['summary.json','oracles.json']),(r,['mac-validation.json'])]:
 for name in names:
  d=json.loads((base/name).read_text());assert d['status']=='success' and d['tests_failed']==0
(r/'diagnostic02').mkdir()
(r/'diagnostic02/complete.json').write_text(json.dumps(dict(all_fits_passed=True,linux_job='7763383',mac='mac-validation.json'))+'\n')
PY
qacct -j 7763387 > "$RUN_ROOT/proof-accounting.txt"
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import json,sys
root=Path(sys.argv[1]);r=Path('/projectnb/welfgr/fereg/dev/20260928-coefficient-stop-proof02')
f=dict(l.split(None,1) for l in (root/'proof-accounting.txt').read_text().splitlines() if l.split() and not l.startswith('='))
assert f['failed'].strip()==f['exit_status'].strip()=='0'
d=json.loads((r/'results/complete.json').read_text())
assert d['complete'] and d['oracle_pass'] and d['candidate_science'] and not d['baseline_science']
assert 'FEREG_PPML_STOP_REPAIR_SUCCESS' in (r/'job.log').read_text()
p=root/'diagnostic02/complete.json';d=json.loads(p.read_text());d.update(scope='Candidate validation and exact-input oracle only; original baseline accuracy failure deliberately retained.',original_baseline_accuracy=False,proof_job='7763387');p.write_text(json.dumps(d)+'\n')
GATE
"$reference/env/python/bin/python" "$RUN_ROOT/rotation03-harness/performance/launch_rotation.py" \
 --run-root "$RUN_ROOT" --attempt rotation03 --baseline "$baseline" \
 --candidate "$candidate/source" --reference "$reference" --scratch "$TMPDIR/coefficient-stop"
printf 'FEREG_COEFFICIENT_STOP_ROTATION_SUCCESS job=%s\n' "$JOB_ID"
