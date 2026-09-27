"""Replay from repository root; STATA_BIN selects the licensed local executable.
Requires the recorded candidate03 source/plugin and original hash-matched inputs.
Outputs are exclusive; move to a new attempt before any deliberate replay.
"""
from pathlib import Path
import hashlib,json,os,re,signal,subprocess,sys,time
repo=Path.cwd();r=repo/'target/performance-repair-20260927'
sys.path[:0]=[str(repo/'benchmarks/systematic'),str(repo/'benchmarks/performance')]
from generate import generate
from diagnose import stata_script
manifest=json.loads((repo/'benchmarks/systematic/manifest-light-v2.json').read_text())
registration=json.loads((repo/'benchmarks/reports/development/separation-coefficient-proof-local-2026-09-27/registration.json').read_text())
source=json.loads((repo/'benchmarks/reports/development/separation-coefficient-proof-local-2026-09-27/candidate03-source.json').read_text())
assert all(hashlib.sha256((repo/f).read_bytes()).hexdigest()==h for f,h in source['files'].items())
plugin_hash=hashlib.sha256((repo/'dist/fereg_macos.plugin').read_bytes()).hexdigest()
base=json.loads((repo/'benchmarks/reports/development/performance-repair-diagnostics-2026-09-27/baseline-plan.json').read_text())
cases=base['cases'][:2]
for fresh in registration['fresh_cases']:
 cell=dict(fresh,model='ppml',section='zeros',effects=manifest['designs'][fresh['design']]['effects']);seed=cell.pop('seed')
 cases.append({'id':f"fresh-{cell['design']}-n{cell['n']}-t{cell['threads']}-{cell['memory']}",'cell':cell,'seed':seed,'solver':'auto'})
results=[]
for case in cases:
 out=r/'coefficient-proof03-probes'/case['id'];out.mkdir(parents=True)
 if case['id'].startswith('fresh'):
  data=out/'input.dta';info=generate(case['cell'],case['seed'],data,manifest)
 else:
  data=r/'original-zero-data'/(case['id']+'.dta');info=json.loads(data.with_suffix('.json').read_text())
 assert hashlib.sha256(data.read_bytes()).hexdigest()==info['sha256']
 (out/'input.json').write_text(json.dumps(info,indent=2)+'\n')
 (out/'fit.do').write_text(stata_script(repo,repo/'references',data,out,case))
 env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')};env['FEREG_SOLVER_TRACE']='1';env['FEREG_SOLVER_TRACE_FILE']=str(out/'solver.trace')
 start=time.monotonic();timeout=False
 with (out/'stdout.txt').open('w') as stdout,(out/'stderr.txt').open('w') as stderr:
  process=subprocess.Popen([os.environ['STATA_BIN'],'-b','do',str(out/'fit.do')],cwd=out,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
  while True:
   pid,status,usage=os.wait4(process.pid,os.WNOHANG)
   if pid:break
   if time.monotonic()-start>180:
    os.killpg(process.pid,signal.SIGKILL);pid,status,usage=os.wait4(process.pid,0);timeout=True;break
   time.sleep(.02)
  rc=os.waitstatus_to_exitcode(status);process.returncode=rc
 meta={}
 if (out/'metadata.tsv').exists():
  for line in (out/'metadata.tsv').read_text().splitlines():
   k,v=line.split('\t',1);meta[k]=float(v)
 log=(out/'fit.log').read_text(errors='replace') if (out/'fit.log').exists() else ''
 failures=[s for s in log.splitlines() if s.startswith('fereg:')]
 rss=re.search(r'(\d+)\s+maximum resident set size', (out/'stderr.txt').read_text(errors='replace'))
 result={'case':case,'plugin_sha256':plugin_hash,'input_sha256':info['sha256'],'process_rc':rc,'timeout':timeout,'process_rss_bytes':usage.ru_maxrss,'memory_source':'macOS wait4 direct-child peak RSS','process_seconds':time.monotonic()-start,'pass':rc==0 and meta.get('rc')==0 and 'PERFORMANCE_COMPLETE rc=0' in log,'failures':failures,'metadata':meta}
 results.append(result);(repo/'benchmarks/reports/development/separation-coefficient-proof-local-2026-09-27/candidate03-results.json').write_text(json.dumps(results,indent=2)+'\n')
 print({k:v for k,v in result.items() if k!='metadata'},flush=True)
