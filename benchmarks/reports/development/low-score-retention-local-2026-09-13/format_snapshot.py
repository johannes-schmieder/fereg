import os
from pathlib import Path
import subprocess,difflib,shutil,json,hashlib
repo=Path(os.environ['REPOSITORY_ROOT']);root=Path(__file__).resolve().parent
paths=subprocess.check_output(['git','diff','--name-only','--','crates'],cwd=repo,text=True).splitlines();assert len(paths)==5
(root/'implementation-before-format.patch').write_bytes(subprocess.check_output(['git','diff','--','crates'],cwd=repo))
fmt=root/'format';fmt.mkdir()
for name in paths:
 p=repo/name;before=p.read_text().splitlines(True);base=subprocess.check_output(['git','show','HEAD:'+name],cwd=repo,text=True).splitlines(True)
 changed=[(max(0,b0-1),b1+1) for tag,a0,a1,b0,b1 in difflib.SequenceMatcher(a=base,b=before,autojunk=False).get_opcodes() if tag!='equal']
 q=fmt/p.name;q.write_text(''.join(before));subprocess.run([os.environ['RUSTFMT_PATH'],'--edition','2021','--config','skip_children=true',str(q)],check=True)
 after=q.read_text().splitlines(True);selected=[]
 for tag,a0,a1,b0,b1 in difflib.SequenceMatcher(a=before,b=after,autojunk=False).get_opcodes():
  if tag!='equal' and any(a0<=hi and a1>=lo for lo,hi in changed):selected.append((a0,a1,after[b0:b1]))
 for a0,a1,replacement in reversed(selected):before[a0:a1]=replacement
 p.write_text(''.join(before))
# This old cache-limit seam is now only used to exercise the uncached oracle arm.
p=repo/'crates/fereg-core/src/ols/prepared_fit.rs';s=p.read_text();needle='pub(crate) fn fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit<';assert s.count(needle)==1;s=s.replace(needle,'#[cfg(test)]\n'+needle);p.write_text(s)
ids=json.loads((root/'baseline-identities.json').read_text());candidate=root/'candidate';shutil.copytree(root/'baseline',candidate)
for name in paths:shutil.copy2(repo/name,candidate/name)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids['candidate_inputs']={p:sha(candidate/p) for p in ids['inputs']};ids['changed_inputs']={p:h for p,h in ids['candidate_inputs'].items() if ids['inputs'][p]!=h};assert len(ids['changed_inputs'])==5
(root/'prebuild-identities.json').write_text(json.dumps(ids,indent=2)+'\n');(root/'implementation.patch').write_bytes(subprocess.check_output(['git','diff','--','crates'],cwd=repo))
print('Isolated candidate snapshot',len(ids['candidate_inputs']),'inputs',len(ids['changed_inputs']),'changed')
