import os
from pathlib import Path
import subprocess,os,json,hashlib,re
r=Path(__file__).resolve().parent;source=r/'candidate';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'prebuild-identities.json').read_text());assert all(sha(source/p)==h for p,h in ids['candidate_inputs'].items())
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(r/'test-target'),RAYON_NUM_THREADS='4')
checks=[('rhs','fereg-core','ols::prepared_rhs::tests'),('categorical','fereg-core','prepared_categorical'),('low','fereg-core','ols::prepared_fit::tests::low_'),('numeric-control','fereg-core','numeric_tiles_preserve'),('plugin-serial','fereg-plugin','')];rows=[]
for name,pkg,pattern in checks:
 cmd=['cargo','test','--locked','-p',pkg,'--lib',pattern,'--','--test-threads=1']
 with (r/(name+'-tests.log')).open('wb') as log:p=subprocess.run(cmd,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
 txt=(r/(name+'-tests.log')).read_text();row={'name':name,'command':cmd,'rc':p.returncode,'summaries':re.findall(r'^test result: .*$',txt,re.M),'log_sha256':sha(r/(name+'-tests.log'))};rows.append(row);(r/'test-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,p.returncode,row['summaries'][-1:],flush=True);assert p.returncode==0 and row['summaries'] and not row['summaries'][-1].startswith('test result: ok. 0 passed'),name
print('GENERATED_RHS_TESTS_TERMINAL_PASS',flush=True)
