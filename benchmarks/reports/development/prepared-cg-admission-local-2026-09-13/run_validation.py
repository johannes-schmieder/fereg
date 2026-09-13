import os
from pathlib import Path
import json,hashlib,subprocess,os,re
root=Path(os.environ['EXPERIMENT_ROOT']);repo=Path(os.environ['REPOSITORY_ROOT']);baseline=Path(os.environ['ACCEPTED_SOURCE'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'cg-identities.json').read_text());log=(root/'build-fix.log').read_text()
assert log.count('Compiling fereg-core')==2 and log.count('Compiling fereg-plugin')==2 and 'actual-plugin ABI/self-test harness: PASS' in log
assert all(sha(root/'candidate'/p)==h for p,h in ids['candidate_inputs'].items())
ids['candidate_plugin']=sha(root/'candidate/dist/fereg_macos.plugin');(root/'cg-built-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
execution=212992;cg=4096*1*8*3+4*1*16+8+128;assert cg==98504
exact=536870912+execution+cg;assert exact==537182408
results=[]
for name,source,cap,rc in [('baseline-uncapped',baseline,None,0),('baseline-below',baseline,exact-1,0),('candidate-below',root/'candidate',exact-1,909),('candidate-exact',root/'candidate',exact,0)]:
 d=root/'validation'/name;d.mkdir(parents=True,exist_ok=False)
 (d/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{root}/cg_cap.do" {rc}\nlocal actual=_rc\ndisplay "CG_VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
 env=os.environ.copy()
 for k in list(env):
  if k.startswith('FEREG_'):del env[k]
 if cap:env['FEREG_MEMORY_LIMIT_BYTES']=str(cap)
 p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(d/'run.do')],cwd=d,env=env,capture_output=True,timeout=120)
 (d/'process.stdout').write_bytes(p.stdout);(d/'process.stderr').write_bytes(p.stderr);txt=(d/'run.log').read_text(errors='replace')
 r={'name':name,'artifact':sha(source/'dist/fereg_macos.plugin'),'available_cap':cap,'usable_cap':None if cap is None else cap-536870912,'expected_estimation_rc':rc,'process_rc':p.returncode,'terminal_rc':re.findall(r'^CG_VALIDATION_TERMINAL_RC=(\d+)\s*$',txt,re.M),'sentinel':bool(re.search(r'^CG_CAP_PASS rc=0\s*$',txt,re.M)),'cg_admission_error':'prepared CG does not fit the joint execution/CG memory plan' in txt,'native_peak':re.findall(r'^CG_CAP_PEAK=\s*([\d.]+)\s*$',txt,re.M),'log_sha256':sha(d/'run.log')}
 results.append(r);(root/'cg-cap-validation.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(r),flush=True)
 assert p.returncode==0 and r['terminal_rc']==['0'] and r['sentinel'],name
 if rc==909:assert r['cg_admission_error'],name
 if name=='candidate-exact':assert float(r['native_peak'][0])<=execution+cg,r
fixture=(repo/'tests/stata/development/within_weighted_ols.do').read_text().replace('solver(lsmr) preconditioner(schwarz)','solver(cg)').replace('END_FIXTURE|within_weighted_ols|','END_FIXTURE|within_weighted_cg|')
(root/'within_weighted_cg.do').write_text(fixture)
for name,fixture,arg,sentinel in [('saved',Path(os.environ['SAVED_ORACLE_FIXTURE']),'0','SAVED_SCHUR_ORACLE|fits=48|rollback=1|rc=0'),('weighted_cg',root/'within_weighted_cg.do','','END_FIXTURE|within_weighted_cg|rc=0')]:
 d=root/'validation'/name;d.mkdir(exist_ok=False);source=root/'candidate'
 (d/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}" {arg}\nlocal actual=_rc\ndisplay "CG_ORACLE_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
 env=os.environ.copy()
 for k in list(env):
  if k.startswith('FEREG_'):del env[k]
 p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(d/'run.do')],cwd=d,env=env,capture_output=True,timeout=180)
 (d/'process.stdout').write_bytes(p.stdout);(d/'process.stderr').write_bytes(p.stderr);txt=(d/'run.log').read_text(errors='replace')
 r={'name':name,'process_rc':p.returncode,'sentinel':bool(re.search('^'+re.escape(sentinel)+r'\s*$',txt,re.M)),'log_sha256':sha(d/'run.log'),'fixture_sha256':sha(fixture)}
 results.append(r);(root/'cg-validation.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(r),flush=True);assert p.returncode==0 and r['sentinel'],name
