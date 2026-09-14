import os
from pathlib import Path
import subprocess,os,json,re,hashlib
r=Path(__file__).resolve().parent;checks=json.loads((r/'test-results.json').read_text());assert len(checks)==7 and all(x['rc']==0 for x in checks)
names=set()
for item in checks:
 if item['command'][4]=='fereg-core':names.update(re.findall(r'^test ([\w:]+) \.\.\. ok$',(r/(item['name']+'-tests.log')).read_text(),re.M))
(r/'validation-summary.json').write_text(json.dumps({'focused_core_distinct_count':len(names),'core_tests':sorted(names),'checks':checks,'status':'revised warning/fallback tests pass; subsequent stages pending'},indent=2)+'\n')
repo=Path(os.environ['REPOSITORY_ROOT']);env=os.environ.copy();env.update(STATA_MP_PATH=os.environ['STATA_MP_PATH'],FEREG_LOCAL_BENCH_ROOT=str(r),PYTHONPATH=os.pathsep.join(str(repo/p) for p in ['benchmarks/local_fixest_gap','benchmarks/systematic']))
rows=[]
for stage in ['build','oracle','screen','followup','map_refs','traces','public_boundaries']:
 with (r/(stage+'-run.log')).open('wb') as log:result=subprocess.run([os.sys.executable,str(r/('run_'+stage+'.py'))],env=env,stdout=log,stderr=subprocess.STDOUT)
 row={'stage':stage,'rc':result.returncode,'log_sha256':hashlib.sha256((r/(stage+'-run.log')).read_bytes()).hexdigest()};rows.append(row);(r/'pipeline-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(stage,result.returncode,flush=True);assert result.returncode==0,stage
print('FINAL_SCHUR_PIPELINE_TERMINAL_PASS',flush=True)
