from pathlib import Path
import os
import json,copy,hashlib
import numpy as np
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
root=Path(os.environ['EXPERIMENT_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
old=Path(os.environ['CG_REPAIR_RUNS'])
original=old/'control-unit_saved-s104729-candidate/request.json'
recheck=root/'recheck-unit-s8675309.json'
requests=[('aa_s8675309',recheck),('ab_reversed_s104729',original),('aa_s104729',original),('ab_reversed_s8675309',recheck)]
ids=json.loads((root/'v2-built-identities.json').read_text())
checks=[]
for i,(name,path) in enumerate(requests):
 req=json.loads(path.read_text());cell=req['cell'];data,warm=Path(req['data']),Path(req['warm'])
 if name=='small_aw':cell.update(absorb='saved_g=g1',fit_options='residuals(saved_e)')
 pairs={}
 for arm in (['candidate','baseline'] if name=='ab_reversed_s104729' else ['baseline','candidate']):
  artifact_arm='baseline' if name.startswith('aa_') else arm
  source=root/('baseline' if artifact_arm=='baseline' else 'candidate-v2');assert sha(source/'dist/fereg_macos.plugin')==ids[artifact_arm+'_plugin']
  out=root/'followup-runs'/f'{name}-{arm}';m=copy.deepcopy(MANIFEST);m['measurement']['command_timeout_seconds']=180
  result=measure(root,cell,'fereg','warm',data,warm,out,m,source=source,identity={'artifact':ids[artifact_arm+'_plugin'],'purpose':'incremental budget cost screen','protocol_sha256':sha(root/'cost_followup_protocol.md'),'harness_sha256':sha(Path(__file__))})
  print(json.dumps({'name':name,'arm':arm,'status':result['status'],'seconds':result['seconds']}),flush=True)
  assert result['status']=='pass',str(out)
  (out/'native-science.json').write_text(json.dumps(certify(out),indent=2)+'\n');pairs[arm]=out
 comparison=baseline_compare(pairs['candidate'],pairs['baseline'])
 for file in ['b','V','fereg_status','fereg_dof','fereg_fit','fereg_certificate','fereg_fe','fereg_cluster','fereg_subset','saved_outputs']:
  a,b=(pairs[arm]/(file+'.csv') for arm in ['baseline','candidate']);assert a.exists()==b.exists(),file
  if a.exists():assert a.read_bytes()==b.read_bytes(),(name,file)
 cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15]
 assert np.array_equal(matrix(pairs['baseline'],'fereg_solver')[:,cols],matrix(pairs['candidate'],'fereg_solver')[:,cols]),name
 a,b=(json.loads((pairs[arm]/'result.json').read_text()) for arm in ['baseline','candidate']);delta=b['seconds']-a['seconds']
 row=dict(name=name,baseline=a['seconds'],candidate=b['seconds'],delta=delta,percent=100*delta/a['seconds'],review=delta>max(.010,.05*a['seconds']),science_exact=True,baseline_metadata=a['metadata'],candidate_metadata=b['metadata'])
 checks.append(row);(root/'followup-results.json').write_text(json.dumps(checks,indent=2)+'\n');print(json.dumps({k:v for k,v in row.items() if not k.endswith('metadata')}),flush=True)
