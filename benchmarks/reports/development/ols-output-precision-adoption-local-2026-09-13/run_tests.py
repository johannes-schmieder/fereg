import os
from pathlib import Path
import os,subprocess,json,re,hashlib
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'prebuild-identities.json').read_text())
assert all(sha(root/'candidate'/p)==h for p,h in ids['candidate_inputs'].items())
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(root/'test-target'),RAYON_NUM_THREADS='4')
results=[]
for package,name,label in [('fereg-core','ols::prepared_fit::tests','prepared-fit'),('fereg-core','fe::cmg::tests','cmg'),('fereg-core','fe::auto::','generic-auto'),('fereg-plugin','','plugin-serial')]:
 command=['cargo','test','--locked','-p',package,'--lib',name,'--','--test-threads=1']
 with (root/(label+'-tests.log')).open('wb') as log:
  p=subprocess.run(command,cwd=root/'candidate',env=env,stdout=log,stderr=subprocess.STDOUT)
 txt=(root/(label+'-tests.log')).read_text();summaries=re.findall(r'^test result: .+$',txt,re.M)
 row=dict(label=label,command=command,rc=p.returncode,summaries=summaries,log_sha256=sha(root/(label+'-tests.log')))
 results.append(row);(root/'test-results.json').write_text(json.dumps(results,indent=2)+'\n');print(row,flush=True);assert p.returncode==0
print('FOCUSED_TESTS_TERMINAL_PASS',flush=True)
