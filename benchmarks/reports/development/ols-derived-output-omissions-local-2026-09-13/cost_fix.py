import os
from pathlib import Path
import json,copy,hashlib
import numpy as np
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
root=Path(os.environ['OMISSION_ROOT']);old=Path(os.path.join(os.environ['CG_ADMISSION_ROOT'], 'cost-runs'))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'identities.json').read_text())
validation=json.loads((root/'validation-results.json').read_text());assert len(validation)==3 and all(r['sentinel'] for r in validation)
original=json.loads((root/'original-results.json').read_text());assert len(original)==2 and all(r['status']=='pass' for r in original)
requests=[(name,old/f'{name}-candidate/request.json') for name in ['unit_saved','pw_partial','weak_d3','low_fw','small_aw','complete_iid']]
assert all(p.exists() for _,p in requests)
(root/'cost-requests.json').write_text(json.dumps({n:json.loads(p.read_text()) for n,p in requests},indent=2)+'\n')
checks=[]
def pair(name,req,arms,suffix=''):
 pairs={}
 for arm in arms:
  source=root/arm;assert sha(source/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
  out=root/'cost-runs'/f'{name}{suffix}-{arm}';m=copy.deepcopy(MANIFEST);m['measurement']['command_timeout_seconds']=180
  result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,m,source=source,identity={'artifact':ids[arm+'_plugin'],'purpose':'isolated derived-output omission repair cost','protocol':sha(root/'protocol.md')})
  print(json.dumps({'name':name+suffix,'arm':arm,'status':result['status'],'seconds':result['seconds']}),flush=True);assert result['status']=='pass',out
  (out/'native-science.json').write_text(json.dumps(certify(out),indent=2)+'\n');pairs[arm]=out
 comparison=baseline_compare(pairs['candidate'],pairs['baseline'])
 for file in ['b','V','fereg_status','fereg_dof','fereg_fit','fereg_certificate','fereg_fe','fereg_cluster','fereg_subset','saved_outputs']:
  a,b=(pairs[arm]/(file+'.csv') for arm in ['baseline','candidate']);assert a.exists()==b.exists(),file
  if a.exists():assert a.read_bytes()==b.read_bytes(),(name,file)
 cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15]
 assert np.array_equal(matrix(pairs['baseline'],'fereg_solver')[:,cols],matrix(pairs['candidate'],'fereg_solver')[:,cols]),name
 a,b=(json.loads((pairs[arm]/'result.json').read_text()) for arm in ['baseline','candidate']);delta=b['seconds']-a['seconds']
 row=dict(name=name+suffix,baseline=a['seconds'],candidate=b['seconds'],delta=delta,percent=100*delta/a['seconds'],review=delta>max(.010,.05*a['seconds']),science_exact=True,baseline_metadata=a['metadata'],candidate_metadata=b['metadata'],comparison=comparison)
 checks.append(row);(root/'cost-fix-results.json').write_text(json.dumps(checks,indent=2)+'\n');print(json.dumps({k:v for k,v in row.items() if not k.endswith('metadata') and k!='comparison'}),flush=True)
 return row
flagged=[]
for i,(name,path) in enumerate(requests):
 req=json.loads(path.read_text());arms=['baseline','candidate'] if i%2==0 else ['candidate','baseline'];row=pair(name,req,arms)
 if row['review']:flagged.append((name,req,arms))
for name,req,arms in flagged:
 for repeat in [1,2]:pair(name,req,list(reversed(arms)),f'-recheck{repeat}')
