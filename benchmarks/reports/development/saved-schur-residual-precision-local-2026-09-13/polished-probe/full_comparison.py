import os
from pathlib import Path
import json,hashlib
from compare import compare_pair
ROOT=Path(__file__).parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((ROOT/'built-identities.json').read_text())
for arm in ['baseline','candidate']:
 assert sha(ROOT/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
 assert all(sha(ROOT/arm/p)==h for p,h in ids[arm+'_inputs'].items())
assert all(r['sentinel'] for r in json.loads((ROOT/'validation-results.json').read_text()))
caps=json.loads((ROOT/'initial-cap-results.json').read_text());assert len(caps)==4 and all(r['rc']==(0 if r['offset']==0 else 909) for r in caps)
assert json.loads((ROOT/'omission-results.json').read_text())['sentinel']
rank=json.loads((ROOT/'rank-replay-result.json').read_text());assert rank['status']=='pass' and all(x['failing_rows']==0 for x in rank['outputs'].values())
assert all(r['rc']==0 for r in json.loads((ROOT/'test-results.json').read_text()))
assert '1 passed; 0 failed' in (ROOT/'green.log').read_text()
for kind,file in [('primary','primary-requests.json'),('control','control-requests.json')]:
 for index,request in enumerate(json.loads((ROOT/file).read_text())):
  row=compare_pair(request,index,kind)
  if row['review']:
   for repeat in [1,2]:compare_pair(request,index+1,'recheck',suffix=f'-{repeat}')
print('SAVED_SCHUR_POLISH_COMPARISON_TERMINAL_PASS',flush=True)
