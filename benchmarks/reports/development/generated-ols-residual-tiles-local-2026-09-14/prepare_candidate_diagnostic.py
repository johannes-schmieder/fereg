import os
from pathlib import Path
import json,hashlib,shutil,subprocess
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());src=r/'candidate';dest=r/'diagnostic-candidate';assert all(sha(src/p)==h for p,h in ids['candidate_inputs'].items());shutil.copytree(src,dest,ignore=shutil.ignore_patterns('dist','target'))
s=(r/'prepare_diagnostic.py').read_text();a=s.index("rel='crates/fereg-core/src/ols/prepared_fit.rs'");b=s.index("(r/'diagnostic-identities.json')",a);fragment=s[a:b].replace('diagnostic-baseline.patch','diagnostic-candidate.patch');exec(fragment)
records=json.loads((r/'diagnostic-identities.json').read_text());records['candidate']={'inputs':{p:sha(dest/p) for p in ids['candidate_inputs']},'original_plugin':ids['candidate_plugin'],'source_patch_sha256':sha(r/'diagnostic-candidate.patch')};(r/'diagnostic-identities.json').write_text(json.dumps(records,indent=2)+'\n');print('CANDIDATE_DIAGNOSTIC_PREPARED')
