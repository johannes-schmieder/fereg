import os
from pathlib import Path
import subprocess,os,json,hashlib,re
r=Path(__file__).resolve().parent;source=r/'candidate';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'prebuild-identities.json').read_text());assert all(sha(source/p)==h for p,h in ids['candidate_inputs'].items())
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(r/'test-target'),RAYON_NUM_THREADS='4')
checks=[('memory', 'fereg-core', 'ppml::memory::tests'), ('family', 'fereg-core', 'ppml::family::tests'), ('caller-rollback', 'fereg-core', 'ppml::prepared_irls::tests::strict_caller'), ('checkpoint', 'fereg-core', 'ppml::prepared_irls::tests::strict_checkpoint_movement'), ('identical-frame', 'fereg-core', 'ppml::prepared_irls::tests::bitwise_identical_weights'), ('warm-fallback', 'fereg-core', 'ppml::prepared_irls::tests::invalid_warm_projection'), ('plugin-serial', 'fereg-plugin', '')];rows=[]
for name,pkg,pattern in checks:
 cmd=['cargo','test','--locked','-p',pkg,'--lib',pattern,'--','--test-threads=1']
 with (r/(name+'-tests.log')).open('wb') as log:p=subprocess.run(cmd,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
 txt=(r/(name+'-tests.log')).read_text();row={'name':name,'command':cmd,'rc':p.returncode,'summaries':re.findall(r'^test result: .*$',txt,re.M),'log_sha256':sha(r/(name+'-tests.log'))};rows.append(row);(r/'test-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,p.returncode,row['summaries'][-1:],flush=True);assert p.returncode==0 and row['summaries'] and not row['summaries'][-1].startswith('test result: ok. 0 passed'),name
print('MEAN_TESTS_TERMINAL_PASS',flush=True)
