import os
from pathlib import Path
import json,copy,hashlib
from preparation_compare import input_for
from run import MANIFEST
root=Path(os.environ['SAVED_SCREEN_ROOT'])
PRIMARY=[dict(name='unit_saved',design='strong_d2',n=245760,k=50,threads=16,absorb='saved_g=g1 saved_h=g2',fit_options='residuals(saved_e)'),dict(name='aw_saved',design='strong_d2',n=983040,k=50,threads=4,weight='[aw=w]',absorb='saved_g=g1 saved_h=g2',fit_options='residuals(saved_e)'),dict(name='pw_partial',design='strong_d2',n=245760,k=50,threads=16,weight='[pw=w]',vce='cluster c1 c2',missing=True,absorb='saved_g=g1 g2',fit_options='residuals(saved_e)')]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
requests=[]
for seed in [104729,8675309,20260819]:
 for spec in PRIMARY:
  cell=dict(model='ols',section='width',effects=['g1','g2'],**spec)
  if seed==104729:
   old=Path(os.path.join(os.environ['SAVED_ORIGINAL_ROOT'], 'runs'))/f"screen-{spec['name']}-s104729-baseline/request.json"
   request=json.loads(old.read_text());assert request['cell']==cell,(request['cell'],cell)
   data,warm=Path(request['data']),Path(request['warm'])
  elif spec['name']=='unit_saved':
   old=Path(os.path.join(os.environ['RECOVERY_ROOT'], 'preparation-data'))
   data=old/f"unit_saved-n245760-s{seed}.dta";warm=old/f"unit_saved-n7680-s{seed+1}.dta"
   assert data.exists() and warm.exists()
  else:
   data,warm=input_for(spec,seed),input_for(spec,seed+1,True)
  for p in [data,warm]:assert sha(p)==json.loads(p.with_suffix('.json').read_text())['sha256']
  requests.append(dict(name=spec['name'],seed=seed,cell=cell,data=str(data),warm=str(warm),data_sha256=sha(data),warm_sha256=sha(warm)))
  print(json.dumps({'prepared':spec['name'],'seed':seed,'data':data.name}),flush=True)
(root/'primary-requests.json').write_text(json.dumps(requests,indent=2)+'\n')
# Existing cap captures must preserve every full saved/scientific export.
for name in ['unit_saved','pw_partial']:
 old=Path(os.path.join(os.environ['RECOVERY_ROOT'], 'cost-runs'))/(name+'-candidate');actual=root/'initial-caps'/f'{name}-0'
 for file in ['b.csv','V.csv','saved_outputs.csv','fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:
  assert (actual/file).read_bytes()==(old/file).read_bytes(),(name,file)
(root/'cap-full-science.json').write_text(json.dumps({'unit_saved':True,'pw_partial':True,'full_b_V_saved_status_dof_fe_cluster_subset_exact':True},indent=2)+'\n')
