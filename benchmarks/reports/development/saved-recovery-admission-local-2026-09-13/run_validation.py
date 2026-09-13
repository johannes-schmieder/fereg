from pathlib import Path
import os
import subprocess, os, json, hashlib, re
root=Path(os.environ['EXPERIMENT_ROOT']); repo=Path(os.environ['REPOSITORY_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
man=json.loads((root/'v2-source-manifest.json').read_text())
for rel,h in man['inputs'].items():
 assert sha(root/'candidate-v2'/rel)==h,rel
 assert sha(repo/rel)==h,rel
log=(root/'build-v2.log').read_text()
assert log.count('Compiling fereg-core')==2 and log.count('Compiling fereg-plugin')==2
assert 'actual-plugin ABI/self-test harness: PASS' in log
ids={'candidate_plugin':sha(root/'candidate-v2/dist/fereg_macos.plugin'),'baseline_plugin':sha(root/'baseline/dist/fereg_macos.plugin'),'candidate_manifest':sha(root/'v2-source-manifest.json'),'protocol':sha(root/'cap_protocol.md'),'fixture':sha(root/'recovery_cap.do')}
assert ids['baseline_plugin']=='18427c19e6fc5512657092bfe0018b180f4e826f5e6a3ea25869d7644f4f77b5'
(root/'v2-built-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
print(json.dumps(ids),flush=True)
results=[]
for name,arm,cap,rc in [('baseline-uncapped','baseline',None,0),('baseline-below','baseline',537252607,0),('candidate-below','candidate-v2',537252607,909),('candidate-exact','candidate-v2',537252608,0)]:
 d=root/name; d.mkdir(exist_ok=False)
 (d/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{root/arm}/stata"\nadopath ++ "{root/arm}/dist"\ncapture noisily do "{root}/recovery_cap.do" {rc}\nlocal actual=_rc\ndisplay "VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
 env=os.environ.copy()
 for k in list(env):
  if k.startswith('FEREG_'):del env[k]
 if cap: env['FEREG_MEMORY_LIMIT_BYTES']=str(cap)
 p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(d/'run.do')],cwd=d,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=120)
 (d/'process.log').write_bytes(p.stdout)
 txt=(d/'run.log').read_text(errors='replace')
 r={'name':name,'artifact':ids['baseline_plugin' if arm=='baseline' else 'candidate_plugin'],'cap':cap,'expected_estimation_rc':rc,'process_rc':p.returncode,'terminal_rc':re.findall(r'^VALIDATION_TERMINAL_RC=(\d+)\s*$',txt,re.M),'pass_sentinel':bool(re.search(r'^RECOVERY_CAP_PASS rc=0\s*$',txt,re.M)),'log_sha256':sha(d/'run.log'),'recovery_error':('saved-FE recovery requires at least' in txt)}
 results.append(r); (root/'cap-validation.json').write_text(json.dumps(results,indent=2)+'\n'); print(json.dumps(r),flush=True)
 assert p.returncode==0 and r['terminal_rc']==['0'] and r['pass_sentinel'],name
 if rc==909:assert r['recovery_error'],name
