import os
from pathlib import Path
import json,hashlib,copy
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST
from final_compare import certify,matrix,baseline_compare
ROOT=Path(__file__).parent;OLD=Path(os.environ['FEREG_SAVED_SCHUR_FINAL_20260913_ROOT']);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((ROOT/'built-identities.json').read_text());assert sha(ROOT/'candidate/dist/fereg_macos.plugin')==ids['candidate_plugin']
request=next(x for x in json.loads((ROOT/'control-requests.json').read_text()) if x['name']=='rank_deficient_saved')
assert all(sha(Path(request[key]))==request[key+'_sha256'] for key in ['data','warm'])
out=ROOT/'functional/rank_deficient_saved';settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
result=measure(ROOT,request['cell'],'fereg','warm',Path(request['data']),Path(request['warm']),out,settings,source=ROOT/'candidate',identity={'plugin':ids['candidate_plugin'],'baseline_commit':ids['baseline_commit'],'purpose':'unchanged original failing request; oracle validation outside paired timing aggregates'})
row={'status':result['status'],'metadata':result['metadata'],'artifact':ids['candidate_plugin'],'data_sha256':request['data_sha256']}
(ROOT/'rank-replay-result.json').write_text(json.dumps(row,indent=2)+'\n');assert result['status']=='pass'
row['native_certificates']=certify(out)
truth=pd.read_csv(OLD/'rank-direct-oracle.csv');saved=pd.read_csv(out/'saved_outputs.csv');assert np.array_equal(truth.obs_id,saved.obs_id)
row['outputs']={}
for col in ['saved_e','saved_g','saved_h']:
 error=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy())
 row['outputs'][col]={'maxabs':float(error.max()),'failing_rows':int(np.count_nonzero(error>limit)),'largest_tolerance_ratio':float(np.max(error/limit))}
b=pd.read_csv(OLD/'rank-direct-coefficients.csv').value.to_numpy();V=pd.read_csv(OLD/'rank-direct-covariance.csv').to_numpy()
actual_b=matrix(out,'b').reshape(-1);actual_V=matrix(out,'V');assert np.allclose(actual_b,b,atol=1e-10,rtol=1e-7);assert np.allclose(actual_V,V,atol=1e-10,rtol=1e-6)
row['full_b_maxabs']=float(np.max(np.abs(actual_b-b)));row['full_V_maxabs']=float(np.max(np.abs(actual_V-V)))
old_run=OLD/'runs/control-rank_deficient_saved-s31415927-baseline';row['baseline_comparison']=baseline_compare(out,old_run)
for filename in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:
 assert (out/filename).read_bytes()==(old_run/filename).read_bytes(),filename
row['oracle_hashes']={p.name:sha(p) for p in OLD.glob('rank-direct-*.csv')}
row['solver']=matrix(out,'fereg_solver').tolist();(ROOT/'rank-replay-result.json').write_text(json.dumps(row,indent=2)+'\n')
print(json.dumps({k:v for k,v in row.items() if k not in ['metadata','solver']},indent=2),flush=True)
assert all(x['failing_rows']==0 for x in row['outputs'].values())
print('ORIGINAL_RANK_REPLAY_TERMINAL_PASS',flush=True)
