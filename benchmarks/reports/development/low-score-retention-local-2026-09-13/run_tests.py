import os
from pathlib import Path
import os,json,hashlib,subprocess,shutil,re
root=Path(__file__).resolve().parent;repo=Path(os.environ['REPOSITORY_ROOT']);source=root/'candidate';ids=json.loads((root/'prebuild-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(root/'prebuild-before-copy-counters.json').write_text(json.dumps(ids,indent=2)+'\n')
for p in ids['changed_inputs']:shutil.copy2(repo/p,source/p)
ids['candidate_inputs']={p:sha(source/p) for p in ids['inputs']};ids['changed_inputs']={p:h for p,h in ids['candidate_inputs'].items() if ids['inputs'][p]!=h};(root/'prebuild-identities.json').write_text(json.dumps(ids,indent=2)+'\n');(root/'implementation.patch').write_bytes(subprocess.check_output(['git','diff','--','crates'],cwd=repo))
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(root/'test-target'),RAYON_NUM_THREADS='4')
checks=[('scratch','fereg-core','ols::prepared_scratch::tests'),('robust','fereg-core','ols::prepared_robust::tests'),('clustered','fereg-core','ols::prepared_clustered::tests'),('low-fit','fereg-core','ols::prepared_fit::tests::low_'),('wide-low','fereg-core','ols::prepared_fit::tests::wide_fast_and_low_agree'),('zero-rank','fereg-core','ols::prepared_fit::tests::rank_zero_fast_path'),('plugin-serial','fereg-plugin','')];results=[]
for label,package,pattern in checks:
 cmd=['cargo','test','--locked','-p',package,'--lib',pattern,'--','--test-threads=1']
 with (root/(label+'-tests.log')).open('wb') as log:p=subprocess.run(cmd,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
 txt=(root/(label+'-tests.log')).read_text();row={'label':label,'command':cmd,'rc':p.returncode,'summaries':re.findall(r'^test result: .*$',txt,re.M),'log_sha256':sha(root/(label+'-tests.log'))};results.append(row);(root/'test-results.json').write_text(json.dumps(results,indent=2)+'\n');print(label,row['rc'],row['summaries'][-1:] ,flush=True)
 assert p.returncode==0,label
print('LOW_SCORE_TESTS_TERMINAL_PASS',flush=True)
