import os
from pathlib import Path
import os,json,hashlib,subprocess,re
root=Path(__file__).resolve().parent;source=root/'candidate';repo=Path(os.environ['REPOSITORY_ROOT']);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((root/'built-identities.json').read_text());assert sha(source/'dist/fereg_macos.plugin')==ids['candidate_plugin']
cases=[('low-one-fe',repo/'tests/stata/development/ols_low_one_fe.do','END_FIXTURE|ols_low_one_fe|rc=0'),('low-cluster',repo/'tests/stata/development/ols_cluster_low_memory.do','END_FIXTURE|ols_cluster_low_memory|rc=0'),('two-way',repo/'tests/stata/development/ols_cluster_batch.do','END_FIXTURE|ols_cluster_batch|rc=0'),('layouts',root/'layout_oracle.do','END_FIXTURE|low_score_layout|fits=4|rc=0')];rows=[]
for name,fixture,sentinel in cases:
 out=root/'validation'/name;out.mkdir(parents=True,exist_ok=False)
 (out/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}"\nlocal result=_rc\ndisplay "LOW_SCORE_VALIDATION_RC=`result\'"\nexit `result\'\n')
 env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')};env['FEREG_PHASE_TIMING']='1'
 p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(out/'run.do')],cwd=out,env=env,capture_output=True,timeout=240)
 (out/'process.stdout').write_bytes(p.stdout);(out/'process.stderr').write_bytes(p.stderr);txt=(out/'run.log').read_text(errors='replace')
 row={'name':name,'fixture_sha256':sha(fixture),'plugin':ids['candidate_plugin'],'process_rc':p.returncode,'terminal_rc':re.findall(r'^LOW_SCORE_VALIDATION_RC=(\d+)\s*$',txt,re.M),'sentinel':bool(re.search('^'+re.escape(sentinel)+r'\s*$',txt,re.M)),'log_sha256':sha(out/'run.log'),'stderr_sha256':sha(out/'process.stderr'),'retention_trace':[x for x in p.stderr.decode(errors='replace').splitlines() if x.startswith('FEREG_PHASE|low_score_retention')]}
 rows.append(row);(root/'validation-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,row['process_rc'],row['terminal_rc'],row['sentinel'],len(row['retention_trace']),flush=True)
 assert row['process_rc']==0 and row['terminal_rc']==['0'] and row['sentinel'],name
print('LOW_SCORE_VALIDATION_TERMINAL_PASS',flush=True)
