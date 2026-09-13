import os
from pathlib import Path
import json,hashlib
from compare import compare_pair
ROOT=Path(__file__).parent
OLD=Path(os.environ['SAVED_SCREEN_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((ROOT/'built-identities.json').read_text())
for arm in ['baseline','candidate']:
 assert sha(ROOT/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
 assert all(sha(ROOT/arm/p)==h for p,h in ids[arm+'_inputs'].items())
assert all(r['sentinel'] for r in json.loads((ROOT/'validation-results.json').read_text()))
caps=json.loads((ROOT/'initial-cap-results.json').read_text());assert len(caps)==4 and all(r['rc']==(0 if r['offset']==0 else 909) for r in caps)
extra=json.loads((ROOT/'extra-validation-results.json').read_text());assert len(extra)==4 and extra[0]['sentinel'] and all(r['exact_exports'] for r in extra[1:])
assert all(r['rc']==0 for r in json.loads((ROOT/'test-results.json').read_text()))
provenance=[]
for kind,file in [('control','control-requests.json'),('primary','primary-requests.json')]:
 for index,request in enumerate(json.loads((ROOT/file).read_text())):
  row=compare_pair(request,index,kind)
  if kind=='primary':
   old=OLD/'runs'/f"primary-{request['name']}-s{request['seed']}-candidate"
   new=ROOT/'runs'/f"primary-{request['name']}-s{request['seed']}-candidate"
   check={'name':request['name'],'seed':request['seed'],'exact_exports':{}}
   for filename in ['b.csv','V.csv','fereg_fit.csv','fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv','fereg_certificate.csv','saved_outputs.csv']:
    a,b=old/filename,new/filename;assert a.exists()==b.exists(),(request['name'],filename)
    if a.exists():assert a.read_bytes()==b.read_bytes(),(request['name'],filename);check['exact_exports'][filename]=sha(b)
   provenance.append(check);(ROOT/'primary-provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
  if row['review']:
   for repeat in [1,2]:compare_pair(request,index+1,'recheck',suffix=f'-{repeat}')
print('COMPLETION_COMPARISONS_TERMINAL_PASS',flush=True)
