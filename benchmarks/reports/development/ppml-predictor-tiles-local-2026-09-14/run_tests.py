import os
from pathlib import Path
import subprocess,os,json,hashlib,re
r=Path(__file__).resolve().parent;source=r/'candidate';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'prebuild-identities.json').read_text());assert all(sha(source/p)==h for p,h in ids['candidate_inputs'].items())
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(r/'test-target'),RAYON_NUM_THREADS='4')
checks=[('numeric-rhs','fereg-core','ols::prepared_rhs::tests'),('reconstruction','fereg-core','ols::linear_session::tests'),('session','fereg-core','ols::prepared_fit::tests::certified_linear_session'),('rollback','fereg-core','ols::prepared_fit::tests::strict_rollback'),('team','fereg-core','ols::prepared_fit::tests::retained_fresh_frozen_and_strict'),('identical-frame','fereg-core','ppml::prepared_irls::tests::bitwise_identical_weights'),('caller-rollback','fereg-core','ppml::prepared_irls::tests::strict_caller'),('checkpoint','fereg-core','ppml::prepared_irls::tests::strict_checkpoint_movement'),('plugin-serial','fereg-plugin','')];rows=[]
for name,pkg,pattern in checks:
 cmd=['cargo','test','--locked','-p',pkg,'--lib',pattern,'--','--test-threads=1']
 with (r/(name+'-tests.log')).open('wb') as log:p=subprocess.run(cmd,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
 txt=(r/(name+'-tests.log')).read_text();row={'name':name,'command':cmd,'rc':p.returncode,'summaries':re.findall(r'^test result: .*$',txt,re.M),'log_sha256':sha(r/(name+'-tests.log'))};rows.append(row);(r/'test-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,p.returncode,row['summaries'][-1:],flush=True);assert p.returncode==0,name
print('PREDICTOR_TESTS_TERMINAL_PASS',flush=True)
