import os
from pathlib import Path
import copy,json,hashlib
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST
from final_compare import certify,matrix
root=Path(__file__).resolve().parent;old=Path(os.environ['FEREG_SAVED_RESIDUAL_CORRECTION_20260913_ROOT']);ids=json.loads((root/'built-identities.json').read_text());req=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='low_fw');sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();assert sha(root/'candidate/dist/fereg_macos.plugin')==ids['candidate_plugin'];truth=pd.read_csv(old/'low-fw-direct-oracle.csv');beta=pd.read_csv(old/'low-fw-direct-coefficients.csv').value.to_numpy();V=pd.read_csv(old/'low-fw-direct-covariance.csv').to_numpy();out=root/'functional/low_fw';spec=copy.deepcopy(MANIFEST);spec['measurement']['command_timeout_seconds']=180
result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,spec,source=root/'candidate',identity={'artifact':ids['candidate_plugin'],'purpose':'original low-memory frequency-weighted saved-output oracle'})
row={'status':result['status'],'metadata':result['metadata'],'artifact':ids['candidate_plugin']};(root/'low-fw-replay-results.json').write_text(json.dumps(row,indent=2)+'\n');assert result['status']=='pass';row['certificates']=certify(out);saved=pd.read_csv(out/'saved_outputs.csv');assert np.array_equal(saved.obs_id,truth.obs_id);row['outputs']={}
for col in ['saved_e','saved_g','saved_h']:
 e=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());row['outputs'][col]={'maxabs':float(e.max()),'failing_rows':int(np.count_nonzero(e>limit)),'max_tolerance_ratio':float(np.max(e/limit))}
ab=matrix(out,'b').reshape(-1);av=matrix(out,'V');assert np.allclose(ab,beta,atol=1e-10,rtol=1e-7) and np.allclose(av,V,atol=1e-10,rtol=1e-6);row['b_error']=float(np.max(np.abs(ab-beta)));row['V_error']=float(np.max(np.abs(av-V)));expected=json.loads((old/'low-fw-direct-results.json').read_text());m=result['metadata']
for key,ref in [('N','effective_N'),('df_r','df_r'),('df_a','df_a'),('fereg_df_fit','df_fit'),('fereg_df_correction','df_correction'),('rank','rank')]:assert m[key]==expected[ref]
row['solver']=matrix(out,'fereg_solver').tolist();row['oracle_sha256']=sha(old/'low-fw-direct-oracle.csv');(root/'low-fw-replay-results.json').write_text(json.dumps(row,indent=2)+'\n');print(json.dumps({k:v for k,v in row.items() if k not in ['metadata','solver']},indent=2),flush=True);assert all(x['failing_rows']==0 for x in row['outputs'].values());print('LOW_FW_REPLAY_TERMINAL_PASS',flush=True)
