import os
"""Independent explicit-dummy LSQR/LSMR adjudication of the stopped control."""
from pathlib import Path
import hashlib
import json
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import lsmr, lsqr

ROOT=Path(os.environ['SAVED_SCREEN_ROOT'])
request=next(x for x in json.loads((ROOT/'control-requests.json').read_text()) if x['name']=='weak_d3')
path=Path(request['data'])
assert hashlib.sha256(path.read_bytes()).hexdigest()==request['data_sha256']
frame=pd.read_stata(path,convert_categoricals=False)
n=len(frame); k=request['cell']['k']
X=frame[[f'x{i}' for i in range(1,k+1)]].to_numpy()
y=frame.y.to_numpy()
panels=[sp.csr_matrix(X)]
for name in ['g1','g2']:
    _,codes=np.unique(frame[name],return_inverse=True)
    panels.append(sp.csr_matrix((np.ones(n),(np.arange(n),codes)),shape=(n,codes.max()+1)))
A=sp.hstack(panels,format='csr')
norms=np.sqrt(np.asarray(A.multiply(A).sum(axis=0)).ravel())
scaled=A@sp.diags(1/norms)
answers={}; results={}
for name in ['lsmr','lsqr']:
    if name=='lsmr':
        out=lsmr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,maxiter=20000)
    else:
        out=lsqr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,iter_lim=20000)
    coefficient=out[0]/norms
    residual=y-A@coefficient
    answers[name]=(coefficient,residual)
    results[name]={'stop':int(out[1]),'iterations':int(out[2]),
                   'residual_norm':float(np.linalg.norm(residual)),
                   'scaled_normal_maxabs':float(np.max(np.abs(scaled.T@residual)))}
    assert out[1] in [1,2,4,5], results[name]
    print(name,results[name],flush=True)
results['independent_residual_agreement_maxabs']=float(np.max(np.abs(answers['lsmr'][1]-answers['lsqr'][1])))
assert results['independent_residual_agreement_maxabs']<1e-10
truth=answers['lsmr'][1]
np.savez(ROOT/'weak-oracle-values.npz',obs_id=frame.obs_id.to_numpy(),residual=truth,
         slopes=answers['lsmr'][0][:k],lsqr_residual=answers['lsqr'][1])
for arm in ['baseline','candidate']:
    run=ROOT/'runs'/f'control-weak_d3-s31415927-{arm}'
    saved=pd.read_csv(run/'saved_outputs.csv')
    assert np.array_equal(saved.obs_id,frame.obs_id)
    residual=saved.saved_e.to_numpy()
    delta=np.abs(residual-truth); limit=1e-9+1e-7*np.abs(truth)
    rows=np.flatnonzero(delta>limit)
    slopes=pd.read_csv(run/'coefficients.csv').set_index('name').loc[[f'x{i}' for i in range(1,k+1)],'value'].to_numpy()
    results[arm]={'residual_maxabs':float(delta.max()),'failing_rows':len(rows),
                  'largest_tolerance_ratio':float(np.max(delta/limit)),
                  'slope_maxabs':float(np.max(np.abs(slopes-answers['lsmr'][0][:k]))),
                  'example_failing_obs_ids':saved.obs_id.iloc[rows[:10]].tolist()}
    print(arm,results[arm],flush=True)
(ROOT/'weak-oracle-results.json').write_text(json.dumps(results,indent=2)+'\n')
