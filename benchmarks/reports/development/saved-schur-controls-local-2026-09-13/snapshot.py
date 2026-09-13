import os
from pathlib import Path
import json,hashlib,shutil,subprocess
repo=Path(os.environ['REPOSITORY_ROOT']);root=Path(os.environ['SAVED_SCREEN_ROOT']);previous=Path(os.environ['CG_ADMISSION_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((previous/'cg-built-identities.json').read_text());base=previous/'candidate';assert sha(base/'dist/fereg_macos.plugin')==ids['candidate_plugin']
(root/'baseline').symlink_to(base,target_is_directory=True);candidate=root/'candidate';shutil.copytree(base,candidate)
inputs={}
for p,h in ids['candidate_inputs'].items():
 assert hashlib.sha256(subprocess.check_output(['git','show','HEAD:'+p],cwd=repo)).hexdigest()==h,p
 shutil.copyfile(repo/p,candidate/p);inputs[p]=sha(candidate/p)
changes=[p for p,h in inputs.items() if h!=ids['candidate_inputs'][p]]
assert sorted(changes)==['crates/fereg-core/src/fe/mod.rs','crates/fereg-core/src/memory.rs','crates/fereg-core/src/ols/prepared_categorical_data.rs','crates/fereg-core/src/ols/prepared_fit.rs','crates/fereg-plugin/src/estimate.rs'],changes
(root/'prebuild-identities.json').write_text(json.dumps({'baseline_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'baseline_plugin':ids['candidate_plugin'],'baseline_inputs':ids['candidate_inputs'],'candidate_inputs':inputs,'changes':changes},indent=2)+'\n')
(root/'implementation.patch').write_bytes(subprocess.check_output(['git','diff','--',*changes],cwd=repo))
print(changes)
