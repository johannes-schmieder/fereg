import os
from pathlib import Path
import subprocess,hashlib,json,os,re
r=Path(__file__).resolve().parent;ids=json.loads((r/'built-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();fixture=r/'topology_failure_atomicity.do';rows=[]
for arm in ['baseline','candidate']:
 source=(r/arm).resolve();assert sha(source/'dist/fereg_macos.plugin')==ids[arm+'_plugin'];assert all(sha(source/p)==h for p,h in ids[arm+'_inputs'].items());out=r/('topology-failure-atomicity-'+arm);out.mkdir(exist_ok=False)
 (out/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}"\nlocal result=_rc\ndisplay "GENERATED_ORACLE_RC=`result\'"\nexit `result\'\n')
 env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')};p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(out/'run.do')],cwd=out,env=env,capture_output=True,timeout=300)
 (out/'process.stdout').write_bytes(p.stdout);(out/'process.stderr').write_bytes(p.stderr);txt=(out/'run.log').read_text(errors='replace');row={'arm':arm,'process_rc':p.returncode,'terminal_rc':re.findall(r'^GENERATED_ORACLE_RC=(\d+)\s*$',txt,re.M),'fits':re.findall(r'^GENERATED_ORACLE\|case=(\w+)\|rc=0\s*$',txt,re.M),'sentinel':bool(re.search(r'^END_FIXTURE\|topology_failure_atomicity\|checks=2\|rc=0\s*$',txt,re.M)),'fixture_sha256':sha(fixture),'plugin':ids[arm+'_plugin'],'log_sha256':sha(out/'run.log')};rows.append(row);(r/'topology-failure-atomicity-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(row,flush=True);assert p.returncode==0 and row['terminal_rc']==['0'] and row['sentinel']
print('TOPOLOGY_FAILURE_ATOMICITY_TERMINAL_PASS',flush=True)
