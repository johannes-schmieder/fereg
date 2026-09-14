import os
from pathlib import Path
import copy,json,hashlib
import numpy as np,pandas as pd
import measure as mm
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import named_runtime
from compare_routes import compare_routes
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());req=json.loads((r/'requests.json').read_text())[0]
for arm in ['baseline','candidate']:
 assert all(sha(r/arm/p)==h for p,h in ids[arm+'_inputs'].items());assert sha(r/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
usable=int(json.loads((r/'screen-results.json').read_text())[0]['baseline_runtime']['modeled_selected_bytes']);assert usable==2429462126
lo,hi=usable,2*usable+512*1024**2
while lo<hi:
 mid=(lo+hi)//2
 if mid-max(512*1024**2,mid//5)>=usable:hi=mid
 else:lo=mid+1
available=lo;assert available-max(512*1024**2,available//5)==usable
original_env=mm.thread_environment;rows=[]
for label,cap in [('exact',available),('short',available-1),('saved',None)]:
 def frame_env(threads):
  result=original_env(threads)
  if cap is not None:result['FEREG_MEMORY_LIMIT_BYTES']=str(cap)
  return result
 mm.thread_environment=frame_env;cell=copy.deepcopy(req['cell']);solver='lsmr' if cap else 'auto';paths={}
 if label=='saved':cell.update(absorb='saved_g1=g1 saved_g2=g2',fit_options='mu(saved_mu)')
 for arm in ['baseline','candidate']:
  out=r/'boundary-runs'/(label+'-'+arm);manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
  result=measure(r,cell,'feppml','warm',Path(req['data']),Path(req['warm']),out,manifest,source=r/arm,solver=solver,identity={'artifact':ids[arm+'_plugin'],'protocol':sha(r/'public-boundary-protocol.md'),'available_bytes':cap,'purpose':'Public admission and saved-output supplement'})
  row={'label':label,'arm':arm,'available_bytes':cap,'result':result};rows.append(row);paths[arm]=out;(r/'boundary-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(label,arm,result['status'],result['metadata'].get('rc'),result['seconds'],flush=True)
  if result['status']=='pass':row['runtime']=named_runtime(out)
  if label=='short':assert result['metadata'].get('rc')==909 and result['sentinel']
  else:assert result['status']=='pass'
 if label!='short':
  rows[-1]['science']=compare_routes(paths['baseline'],paths['candidate'])
  if label=='saved':
   a,b=[pd.read_csv(paths[arm]/'saved_outputs.csv') for arm in ['baseline','candidate']];assert list(a)==list(b) and len(a)==len(b);assert np.array_equal(a.obs_id,b.obs_id);assert np.allclose(a,b,rtol=1e-7,atol=1e-9,equal_nan=True)
   rows[-1]['saved_outputs']={'rows':len(a),'columns':list(a),'max_abs_difference':float(np.nanmax(np.abs(a.to_numpy()-b.to_numpy()))),'registered_tolerance_pass':True}
 (r/'boundary-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('CHECKPOINT_PUBLIC_BOUNDARIES_TERMINAL_PASS',flush=True)
