import os
from pathlib import Path
import os,json,hashlib,subprocess,re,shutil
root=Path(os.environ['FEREG_SAVED_SCHUR_POLISH_20260913_ROOT']);previous=Path(os.environ['RECOVERY_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'prebuild-identities.json').read_text());log=(root/'build-isolated.log').read_text();assert log.count('Compiling fereg-core')==2 and log.count('Compiling fereg-plugin')==2 and 'actual-plugin ABI/self-test harness: PASS' in log
assert all(sha(root/'candidate'/p)==h for p,h in ids['candidate_inputs'].items())
ids['candidate_plugin']=sha(root/'candidate/dist/fereg_macos.plugin');ids['protocol']=sha(root/'protocol.md');(root/'built-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
shutil.copyfile(previous/'recovery_cap.do',root/'recovery_cap.do')
results=[]
cases=[('recovery-below','candidate',537252607,root/'recovery_cap.do','909','RECOVERY_CAP_PASS rc=0'),('recovery-exact','candidate',537252608,root/'recovery_cap.do','0','RECOVERY_CAP_PASS rc=0'),('saved-oracle','candidate',None,Path(os.path.join(os.environ['SAVED_REPAIRED_ROOT'], 'oracle.do')),'1','SAVED_SCHUR_ORACLE|fits=48|rollback=1|rc=0')]
for name,arm,cap,fixture,arg,sentinel in cases:
 d=root/'validation'/name;d.mkdir(parents=True,exist_ok=False);source=root/arm
 (d/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}" {arg}\nlocal actual=_rc\ndisplay "VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
 env=os.environ.copy()
 for k in list(env):
  if k.startswith('FEREG_'):del env[k]
 if cap:env['FEREG_MEMORY_LIMIT_BYTES']=str(cap)
 p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(d/'run.do')],cwd=d,env=env,capture_output=True,timeout=180)
 (d/'process.stdout').write_bytes(p.stdout);(d/'process.stderr').write_bytes(p.stderr);txt=(d/'run.log').read_text(errors='replace')
 r={'name':name,'artifact':ids['candidate_plugin'],'available_cap':cap,'arg':arg,'process_rc':p.returncode,'sentinel':bool(re.search('^'+re.escape(sentinel)+r'\s*$',txt,re.M)),'log_sha256':sha(d/'run.log'),'fixture_sha256':sha(fixture),'recovery_error':'saved-FE recovery requires at least' in txt}
 results.append(r);(root/'validation-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(r),flush=True);assert p.returncode==0 and r['sentinel'],name
 if name=='recovery-below':assert r['recovery_error']
