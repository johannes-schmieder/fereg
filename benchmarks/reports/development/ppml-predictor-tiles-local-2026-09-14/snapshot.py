import os
from pathlib import Path
import json,hashlib,subprocess,shutil,difflib
r=Path(__file__).resolve().parent;repo=Path(os.environ['REPOSITORY_ROOT']);paths=subprocess.check_output(['git','diff','--name-only','--','crates'],cwd=repo,text=True).splitlines();assert len(paths)==2
(r/'implementation-before-format.patch').write_bytes(subprocess.check_output(['git','diff','--','crates'],cwd=repo));fmt=r/'format';fmt.mkdir()
for name in paths:
 p=repo/name;before=p.read_text().splitlines(True);base=subprocess.check_output(['git','show','HEAD:'+name],cwd=repo,text=True).splitlines(True);changed=[(max(0,b0-1),b1+1) for tag,a0,a1,b0,b1 in difflib.SequenceMatcher(a=base,b=before,autojunk=False).get_opcodes() if tag!='equal'];q=fmt/p.name;q.write_text(''.join(before));subprocess.run([os.environ['RUSTFMT_PATH'],'--edition','2021','--config','skip_children=true',str(q)],check=True);after=q.read_text().splitlines(True);selected=[]
 for tag,a0,a1,b0,b1 in difflib.SequenceMatcher(a=before,b=after,autojunk=False).get_opcodes():
  if tag!='equal' and any(a0<=hi and a1>=lo for lo,hi in changed):selected.append((a0,a1,after[b0:b1]))
 for a0,a1,replacement in reversed(selected):before[a0:a1]=replacement
 p.write_text(''.join(before))
shutil.copytree(r/'baseline',r/'candidate')
for p in paths:shutil.copy2(repo/p,r/'candidate'/p)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'baseline-identities.json').read_text());ids['candidate_inputs']={p:sha(r/'candidate'/p) for p in ids['inputs']};ids['changed_inputs']={p:h for p,h in ids['candidate_inputs'].items() if ids['inputs'][p]!=h};assert len(ids['changed_inputs'])==2;(r/'prebuild-identities.json').write_text(json.dumps(ids,indent=2)+'\n');(r/'implementation.patch').write_bytes(subprocess.check_output(['git','diff','--','crates'],cwd=repo));print('SNAPSHOT_PASS',len(ids['candidate_inputs']))
