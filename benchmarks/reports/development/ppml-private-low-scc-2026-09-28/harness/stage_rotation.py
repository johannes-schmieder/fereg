from pathlib import Path
import json,hashlib,shutil,subprocess,ast,sys
phase=sys.argv[1];number={'initial':'03','fresh':'04'}[phase]
r=Path('benchmarks/reports/development/ppml-private-low-scc-2026-09-28');s=Path('/tmp/fereg-private-low-rotation'+number);s.mkdir()
linux=json.loads((r/'linux-validation01.json').read_text());mac=json.loads((r/'mac-validation01.json').read_text());assert linux['status']==mac['status']=='success'
candidate=json.loads((r/'candidate-source.json').read_text())
def setup(root,num,phase,scaling=False):
 shutil.copytree('/tmp/fereg-centered-inline-rotation04/rotation04-harness',root/f'rotation{num}-harness')
 shutil.copyfile(r/'candidate-source.json' if scaling else r/'baseline-source.json',root/'source-manifest.json');shutil.copyfile(r/'candidate-source.json',root/'candidate01-source.json')
 plan=json.loads((r/f'{phase}-design-registration.json').read_text());plan.update(source_snapshot=candidate['snapshot'],upstream_linux_validation_job='7763918',plugins=dict(baseline=linux['plugin_sha256'] if scaling else 'f919ea96f5700c1ebbf0df9c2b7215d3b4a2e54f17c2578e3c55e4697efaff83',candidate=linux['plugin_sha256']))
 (root/f'rotation{num}-registration.json').write_text(json.dumps(plan,indent=2)+'\n');shutil.copyfile(root/f'rotation{num}-registration.json',r/f'registration{num}.json')
setup(s,number,phase)
for f in ['mac-validation01.json','linux-validation01.json']:shutil.copyfile(r/f,s/f)
if phase=='fresh':(s/'scaling').mkdir();setup(s/'scaling','05','scaling',True)
job='''#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_private_@@
#$ -pe omp 28
#$ -l num_proc=28,cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-private-low-rotation@@ && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import json,sys,hashlib
r=Path(sys.argv[1]);build=Path('/projectnb/welfgr/fereg/dev/20260928-private-low-01')
for f,digest in json.loads((r/'deployment.json').read_text()).items():assert hashlib.sha256((r/f).read_bytes()).hexdigest()==digest,f
for name in ['mac-validation01.json','linux-validation01.json']:
 d=json.loads((r/name).read_text());assert d['status']=='success' and d['tests_failed']==0 and d['rust_distinct_passed']>=101
for name,count in [('summary.json',1),('oracles.json',6)]:
 d=json.loads((build/'output'/name).read_text());assert d['status']=='success' and d['tests_failed']==0 and d['tests_passed']==count
assert 'FEREG_PRIVATE_LOW_VALIDATION_SUCCESS' in (build/'job.log').read_text()
(r/'diagnostic02').mkdir();(r/'diagnostic02/complete.json').write_text(json.dumps(dict(all_fits_passed=True,linux_validation_job='7763918'))+'\\n')
GATE
"$reference/env/python/bin/python" "$RUN_ROOT/rotation@@-harness/performance/launch_rotation.py" \\
 --run-root "$RUN_ROOT" --attempt rotation@@ \\
 --baseline /projectnb/welfgr/fereg/dev/20260928-private-moments-01/source \\
 --candidate /projectnb/welfgr/fereg/dev/20260928-private-low-01/source \\
 --reference "$reference" --scratch "$TMPDIR/private-moments"
"$reference/env/python/bin/python" "$RUN_ROOT/rotation@@-harness/performance/summarize_rotations.py" "$RUN_ROOT/rotation@@-results" --output "$RUN_ROOT/summary@@"
'''.replace('@@',number)
if phase=='fresh':
 job+='''mkdir "$RUN_ROOT/scaling/diagnostic02"
cp "$RUN_ROOT/diagnostic02/complete.json" "$RUN_ROOT/scaling/diagnostic02/"
"$reference/env/python/bin/python" "$RUN_ROOT/scaling/rotation05-harness/performance/launch_rotation.py" \\
 --run-root "$RUN_ROOT/scaling" --attempt rotation05 \\
 --baseline /projectnb/welfgr/fereg/dev/20260928-private-low-01/source \\
 --candidate /projectnb/welfgr/fereg/dev/20260928-private-low-01/source \\
 --reference "$reference" --scratch "$TMPDIR/private-scaling"
"$reference/env/python/bin/python" "$RUN_ROOT/scaling/rotation05-harness/performance/summarize_rotations.py" "$RUN_ROOT/scaling/rotation05-results" --output "$RUN_ROOT/scaling/summary05"
'''
job+="printf 'FEREG_PRIVATE_LOW_TIMING_SUCCESS job=%s\\n' \"$JOB_ID\"\n"
(s/'job.sh').write_text(job);subprocess.run(['bash','-n',str(s/'job.sh')],check=True)
for part in job.split("<<'")[1:]:
 marker,body=part.split("'\n",1);ast.parse(body.split('\n'+marker+'\n',1)[0])
shutil.copyfile(s/'job.sh',r/'harness'/f'rotation_job{number}.sh')
d={str(p.relative_to(s)):hashlib.sha256(p.read_bytes()).hexdigest() for p in s.rglob('*') if p.is_file()};(s/'deployment.json').write_text(json.dumps(d,indent=2)+'\n');shutil.copyfile(s/'deployment.json',r/f'deployment{number}.json')
print(s)
