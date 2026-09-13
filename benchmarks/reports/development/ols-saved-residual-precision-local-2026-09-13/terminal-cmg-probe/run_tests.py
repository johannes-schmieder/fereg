import os
from pathlib import Path
import os,subprocess,json
root=Path(__file__).resolve().parent
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(root/'test-target'),RAYON_NUM_THREADS='4')
results=[]
for name in ['ols::prepared_fit::tests','fe::cmg::tests']:
 command=['cargo','test','--locked','-p','fereg-core','--lib',name]
 with (root/(name.split('::')[1]+'-tests.log')).open('wb') as log:p=subprocess.run(command,cwd=root/'candidate',env=env,stdout=log,stderr=subprocess.STDOUT)
 results.append(dict(command=command,rc=p.returncode));(root/'test-results.json').write_text(json.dumps(results,indent=2)+'\n');print(results[-1],flush=True);assert p.returncode==0
