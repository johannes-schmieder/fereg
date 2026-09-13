from pathlib import Path
import subprocess,hashlib,json,os
root=Path(__file__).parent
records=json.loads((root/'oracle-results.json').read_text())
for arm in ['baseline','candidate']:
 source=(root/arm).resolve()
 for name,fixture,arg,sentinels in [
  ('ols_saved_schur',Path('PREVIOUS_EXPERIMENT/oracle.do'),'0',['SAVED_SCHUR_ORACLE|fits=48|rollback=1|rc=0','END_FIXTURE|ols_saved_schur|rc=0']),
  ('feppml_one_fe_dummy_oracle',root/'feppml_one_fe_dummy_oracle.do','',['END_FIXTURE|feppml_one_fe_dummy_oracle|rc=0'])]:
  if any(r['arm']==arm and r['fixture']==name for r in records):continue
  out=root/'oracles-reviewed'/arm/name;out.mkdir(parents=True,exist_ok=False)
  worker=out/'run.do'
  worker.write_text(f'''version 19
clear all
set more off
capture adopath - SITE
capture adopath - "."
adopath ++ "{source/'stata'}"
adopath ++ "{source/'dist'}"
do "{fixture}" {arg}
''')
  env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
  with (out/'process.log').open('w') as log:
   p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(worker)],cwd=out,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=300)
  log=(out/'run.log').read_text(errors='replace')
  # Stata pads numeric display; normalize only spaces for the fit-count sentinel.
  assert p.returncode==0 and all(s.replace(' ','') in log.replace(' ','') for s in sentinels),(arm,name,p.returncode)
  records.append(dict(arm=arm,fixture=name,process_rc=p.returncode,sentinels=sentinels,fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),plugin_sha256=hashlib.sha256((source/'dist/fereg_macos.plugin').read_bytes()).hexdigest(),log_sha256=hashlib.sha256((out/'run.log').read_bytes()).hexdigest()))
  (root/'oracle-results.json').write_text(json.dumps(records,indent=2)+'\n')
  print(arm,name,'PASS',flush=True)
