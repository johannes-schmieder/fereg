import os
from pathlib import Path
import json,hashlib
import pandas as pd,numpy as np
from final_compare import matrix
root=Path(__file__).resolve().parent;out=[]
for name in ['disconnected_blocks','unbalanced','weak_d3','auto_cmg_saved']:
 p=root/'control-cap-runs'/name
 oracle=root/'oracle-extension'/(name+'-full-oracle.csv') if name!='auto_cmg_saved' else Path(os.path.join(os.environ['RESIDUAL_ORACLE_ROOT'], 'auto-cmg-direct-oracle.csv'))
 truth=pd.read_csv(oracle).set_index('obs_id');actual=pd.read_csv(p/'saved_outputs.csv').set_index('obs_id');row={'name':name,'oracle_sha256':hashlib.sha256(oracle.read_bytes()).hexdigest(),'outputs':{}}
 for col in actual:
  s=pd.to_numeric(actual[col],errors='coerce').dropna();assert s.index.equals(truth.index);d=np.abs(s.to_numpy()-truth[col].to_numpy());assert np.all(d<=1e-9+1e-7*np.abs(truth[col].to_numpy()));row['outputs'][col]=float(d.max())
 b=pd.read_csv(str(oracle).replace('-oracle.csv','-coefficients.csv')).value.to_numpy();v=pd.read_csv(str(oracle).replace('-oracle.csv','-covariance.csv')).to_numpy();ab=matrix(p,'b').reshape(-1);av=matrix(p,'V');assert ab.shape==b.shape and av.shape==v.shape
 assert np.allclose(ab,b,atol=1e-10,rtol=1e-7) and np.allclose(av,v,atol=1e-10,rtol=1e-6)
 row.update(b_error=float(np.max(np.abs(ab-b))),V_error=float(np.max(np.abs(av-v))),all_pass=True);out.append(row);print(name,'PASS',row['b_error'],row['V_error'])
(root/'control-cap-independent.json').write_text(json.dumps(out,indent=2)+'\n')
