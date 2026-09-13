import os
from pathlib import Path
import copy,json,hashlib
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST
from final_compare import certify,matrix
root=Path(__file__).resolve().parent;old=Path(os.environ['FEREG_SAVED_RESIDUAL_REPAIR_20260913_ROOT']);ids=json.loads((root/'built-identities.json').read_text());req=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='explicit_cg_saved');sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();assert sha(root/'candidate/dist/fereg_macos.plugin')==ids['candidate_plugin'];truth=pd.read_csv(old/'auto-cmg-direct-oracle.csv');beta=pd.read_csv(old/'auto-cmg-direct-coefficients.csv').value.to_numpy();V=pd.read_csv(old/'auto-cmg-direct-covariance.csv').to_numpy();results=[]
for solver in ['auto','cg']:
 out=root/'functional'/('terminal-'+solver);spec=copy.deepcopy(MANIFEST);spec['measurement']['command_timeout_seconds']=180
 result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,spec,source=root/'candidate',solver=solver,identity={'artifact':ids['candidate_plugin'],'purpose':'original CMG retry input; independent accuracy and terminal correction validation'})
 row={'solver':solver,'status':result['status'],'metadata':result['metadata'],'artifact':ids['candidate_plugin']};results.append(row);(root/'terminal-cmg-results.json').write_text(json.dumps(results,indent=2)+'\n');assert result['status']=='pass';assert f'solver({solver})' in result['metadata']['command'];row['certificates']=certify(out)
 actual=pd.read_csv(out/'saved_outputs.csv');assert np.array_equal(actual.obs_id,truth.obs_id);row['outputs']={}
 for col in ['saved_e','saved_g','saved_h']:
  e=np.abs(actual[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());row['outputs'][col]={'maxabs':float(e.max()),'failing_rows':int(np.count_nonzero(e>limit)),'max_tolerance_ratio':float(np.max(e/limit))}
 ab=matrix(out,'b').reshape(-1);av=matrix(out,'V');assert np.allclose(ab,beta,atol=1e-10,rtol=1e-7) and np.allclose(av,V,atol=1e-10,rtol=1e-6);row['b_maxabs']=float(np.max(np.abs(ab-beta)));row['V_maxabs']=float(np.max(np.abs(av-V)));report=matrix(out,'fereg_solver');row['solver_report']=report.tolist();assert report[1,11]==0
 if solver=='auto':assert report[1,1]==4 and report[1,10]==1
 assert all(x['failing_rows']==0 for x in row['outputs'].values());row['oracle_sha256']=sha(old/'auto-cmg-direct-oracle.csv');(root/'terminal-cmg-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({k:v for k,v in row.items() if k!='metadata'},indent=2),flush=True)
print('TERMINAL_CMG_ORACLE_PASS',flush=True)
