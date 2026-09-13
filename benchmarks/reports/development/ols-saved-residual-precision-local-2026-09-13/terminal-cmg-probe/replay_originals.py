import os
from pathlib import Path
import copy,json,hashlib
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST
from final_compare import certify,matrix,baseline_compare
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((root/'built-identities.json').read_text())
assert sha(root/'candidate/dist/fereg_macos.plugin')==ids['candidate_plugin']
cases=[('rank_deficient_saved','control-requests.json',Path(os.environ['FEREG_SAVED_SCHUR_FINAL_20260913_ROOT']),'rank-direct','control-rank_deficient_saved-s31415927-baseline'),('pw_partial','primary-requests.json',Path(os.environ['FEREG_SAVED_SCHUR_POLISH_20260913_ROOT']),'pw-s104729-direct','primary-pw_partial-s104729-baseline')]
results=[]
for name,requestfile,oracle,prefix,baseline in cases:
 req=next(x for x in json.loads((root/requestfile).read_text()) if x['name']==name and (name!='pw_partial' or x['seed']==104729))
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 out=root/'functional'/name;settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
 result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,settings,source=root/'candidate',identity={'artifact':ids['candidate_plugin'],'purpose':'original exact input independently oracle checked; outside timing comparisons'})
 row={'name':name,'status':result['status'],'artifact':ids['candidate_plugin'],'input':req['data_sha256'],'metadata':result['metadata']};results.append(row);(root/'original-replay-results.json').write_text(json.dumps(results,indent=2)+'\n');assert result['status']=='pass',name
 row['certificates']=certify(out);row['baseline_comparison']=baseline_compare(out,oracle/'runs'/baseline)
 truth=pd.read_csv(oracle/(prefix+'-oracle.csv'));saved=pd.read_csv(out/'saved_outputs.csv');previous=pd.read_csv(oracle/'runs'/baseline/'saved_outputs.csv');assert np.array_equal(saved.obs_id,previous.obs_id)
 for col in ['saved_e','saved_g']+(['saved_h'] if name!='pw_partial' else []):
  saved[col]=pd.to_numeric(saved[col],errors='coerce');assert np.array_equal(saved[col].isna(),pd.to_numeric(previous[col],errors='coerce').isna())
 active=saved.loc[saved.saved_e.notna()];assert np.array_equal(active.obs_id,truth.obs_id);row['outputs']={}
 for col in ['saved_e','saved_g']+(['saved_h'] if name!='pw_partial' else []):
  error=np.abs(active[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());row['outputs'][col]={'maxabs':float(error.max()),'failing_rows':int(np.count_nonzero(error>limit)),'max_tolerance_ratio':float(np.max(error/limit))}
 b=pd.read_csv(oracle/(prefix+'-coefficients.csv')).value.to_numpy();V=pd.read_csv(oracle/(prefix+'-covariance.csv')).to_numpy();ab=matrix(out,'b').reshape(-1);av=matrix(out,'V');row['b_error']=float(np.max(np.abs(ab-b)));row['V_error']=float(np.max(np.abs(av-V)))
 assert np.allclose(ab,b,atol=1e-10,rtol=1e-7) and np.allclose(av,V,atol=1e-10,rtol=1e-6)
 for f in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:assert (out/f).read_bytes()==(oracle/'runs'/baseline/f).read_bytes(),(name,f)
 row['oracle_hashes']={p.name:sha(p) for p in oracle.glob(prefix+'-*.csv')};row['solver']=matrix(out,'fereg_solver').tolist();(root/'original-replay-results.json').write_text(json.dumps(results,indent=2)+'\n')
 print(json.dumps({k:v for k,v in row.items() if k not in ['metadata','solver','baseline_comparison']},indent=2),flush=True)
 assert all(x['failing_rows']==0 for x in row['outputs'].values()),name
print('ORIGINAL_REPLAY_TERMINAL_PASS',flush=True)
