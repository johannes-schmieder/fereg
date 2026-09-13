import os
from pathlib import Path
import copy,json,hashlib
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST
from final_compare import certify,matrix
root=Path(__file__).resolve().parent;old=Path(os.environ['FEREG_OUTPUT_RESIDUAL_PRECISION_20260913_ROOT'])
ids=json.loads((root/'built-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(root/'candidate/dist/fereg_macos.plugin')==ids['candidate_plugin']
assert all(sha(root/'candidate'/p)==h for p,h in ids['candidate_inputs'].items())
req=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='weighted_cut')
assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
expected=json.loads((old/'weighted-cut-direct-results.json').read_text());assert req['data_sha256']==expected['input_sha256']
truth=pd.read_csv(old/'weighted-cut-direct-oracle.csv');beta=pd.read_csv(old/'weighted-cut-direct-coefficients.csv').value.to_numpy();V=pd.read_csv(old/'weighted-cut-direct-covariance.csv').to_numpy()
out=root/'functional/weighted_cut';spec=copy.deepcopy(MANIFEST);spec['measurement']['command_timeout_seconds']=180
result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,spec,source=root/'candidate',solver=req.get('solver','auto'),identity={'artifact':ids['candidate_plugin'],'purpose':'original tiny-weight-cut full independent output oracle','harness':sha(Path(__file__))})
row={'status':result['status'],'seconds':result['seconds'],'metadata':result['metadata'],'artifact':ids['candidate_plugin'],'input_sha256':req['data_sha256']}
receipt=root/'weighted-cut-replay-results.json';receipt.write_text(json.dumps(row,indent=2)+'\n');assert result['status']=='pass'
row['certificates']=certify(out);saved=pd.read_csv(out/'saved_outputs.csv');assert np.array_equal(saved.obs_id,truth.obs_id);row['outputs']={}
for col in ['saved_e','saved_g','saved_h']:
 e=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());row['outputs'][col]={'maxabs':float(e.max()),'failing_rows':int(np.count_nonzero(e>limit)),'max_tolerance_ratio':float(np.max(e/limit))}
ab=matrix(out,'b').reshape(-1);av=matrix(out,'V');assert ab.shape==beta.shape and av.shape==V.shape
row['b_error']=float(np.max(np.abs(ab-beta)));row['V_error']=float(np.max(np.abs(av-V)))
row['b_pass']=bool(np.allclose(ab,beta,atol=1e-10,rtol=1e-7));row['V_pass']=bool(np.allclose(av,V,atol=1e-10,rtol=1e-6))
m=result['metadata'];assert m['N']==expected['rows'] and m['df_r']==expected['df_r'] and m['rank']==expected['rank']
row['solver']=matrix(out,'fereg_solver').tolist();row['oracle_sha256']=sha(old/'weighted-cut-direct-oracle.csv');receipt.write_text(json.dumps(row,indent=2)+'\n')
print(json.dumps({k:v for k,v in row.items() if k not in ['metadata','solver']},indent=2),flush=True)
assert row['b_pass'] and row['V_pass'] and all(x['failing_rows']==0 for x in row['outputs'].values())
print('WEIGHTED_CUT_REPLAY_TERMINAL_PASS',flush=True)
