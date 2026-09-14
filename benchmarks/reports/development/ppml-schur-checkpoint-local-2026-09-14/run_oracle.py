import os
from pathlib import Path
import subprocess,hashlib,json,os,re
r=Path(__file__).resolve().parent;ids=json.loads((r/'built-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();source=(r/'candidate').resolve();assert sha(source/'dist/fereg_macos.plugin')==ids['candidate_plugin'];assert all(sha(source/p)==h for p,h in ids['candidate_inputs'].items());out=r/'oracle';out.mkdir(exist_ok=False);fixture=r/'two_fe_oracle.do'
(out/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}"\nlocal result=_rc\ndisplay "SCHUR_ORACLE_RC=`result\'"\nexit `result\'\n')
env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(out/'run.do')],cwd=out,env=env,capture_output=True,timeout=300)
(out/'process.stdout').write_bytes(p.stdout);(out/'process.stderr').write_bytes(p.stderr);txt=(out/'run.log').read_text(errors='replace');row={'process_rc':p.returncode,'terminal_rc':re.findall(r'^SCHUR_ORACLE_RC=(\d+)\s*$',txt,re.M),'fits':re.findall(r'^ORACLE_FIT\|kind=(\w+)\|solver=(\w+)\|rc=0\s*$',txt,re.M),'sentinel':bool(re.search(r'^END_FIXTURE\|two_fe_schur_routes\|rc=0\s*$',txt,re.M)),'fixture_sha256':sha(fixture),'plugin':ids['candidate_plugin'],'log_sha256':sha(out/'run.log')};(r/'oracle-results.json').write_text(json.dumps(row,indent=2)+'\n');print(row,flush=True);assert p.returncode==0 and row['terminal_rc']==['0'] and row['sentinel'] and len(row['fits'])==9
print('SCHUR_ROUTE_ORACLE_TERMINAL_PASS',flush=True)
