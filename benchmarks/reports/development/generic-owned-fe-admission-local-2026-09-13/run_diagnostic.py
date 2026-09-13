import os
from pathlib import Path
import os,json,hashlib,subprocess,re
root=Path(__file__).resolve().parent;ids=json.loads((root/'diagnostic-built-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();source=root/'diagnostic-v2';assert sha(source/'dist/fereg_macos.plugin')==ids['candidate_plugin']
cases=[('generic-diagnostic',root/'generic_cap.do','0',None,'GENERIC_CAP_PASS rc=0')]
results=[]
for name,fixture,arg,cap,sentinel in cases:
 work=root/'validation'/name;work.mkdir(parents=True,exist_ok=False)
 (work/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}" {arg}\nlocal actual=_rc\ndisplay "VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
 env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
 if cap:env['FEREG_MEMORY_LIMIT_BYTES']=str(cap)
 p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(work/'run.do')],cwd=work,env=env,capture_output=True,timeout=180)
 (work/'process.stdout').write_bytes(p.stdout);(work/'process.stderr').write_bytes(p.stderr);txt=(work/'run.log').read_text(errors='replace')
 row={'name':name,'process_rc':p.returncode,'cap':cap,'fixture_sha256':sha(fixture),'log_sha256':sha(work/'run.log'),'sentinel':bool(re.search('^'+re.escape(sentinel)+r'\s*$',txt,re.M)),'terminal_rc':re.findall(r'^VALIDATION_TERMINAL_RC=(\d+)\s*$',txt,re.M),'plugin':ids['candidate_plugin']}
 results.append(row);(root/'generic-diagnostic-results.json').write_text(json.dumps(results,indent=2)+'\n');print(row,flush=True);assert p.returncode==0 and row['sentinel'] and (row['terminal_rc']==['0'] or (name=='saved-oracle' and bool(re.search(r'^END_FIXTURE\|ols_saved_schur\|rc=0\s*$',txt,re.M)))),name
print('VALIDATION_TERMINAL_PASS',flush=True)
