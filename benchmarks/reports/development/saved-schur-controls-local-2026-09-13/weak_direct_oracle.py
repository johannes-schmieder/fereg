import os
"""Grounded explicit-dummy direct solve, with independently normalized FE outputs."""
from pathlib import Path
import json
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import splu
from scipy.linalg import lstsq
from scipy.sparse.csgraph import connected_components

ROOT=Path(os.environ['SAVED_SCREEN_ROOT'])
request=next(x for x in json.loads((ROOT/'control-requests.json').read_text()) if x['name']=='weak_d3')
frame=pd.read_stata(request['data'],convert_categoricals=False)
n=len(frame); k=request['cell']['k']; rows=np.arange(n)
_,left=np.unique(frame.g1,return_inverse=True)
_,right=np.unique(frame.g2,return_inverse=True)
D1=sp.csr_matrix((np.ones(n),(rows,left)))
D2=sp.csr_matrix((np.ones(n),(rows,right)))
n1=np.asarray(D1.sum(axis=0)).ravel(); n2=np.asarray(D2.sum(axis=0)).ravel()
E=D1.T@D2
S=sp.diags(n2)-E.T@sp.diags(1/n1)@E
components,labels=connected_components(S,directed=False)
assert components==1, components
factor=splu(S[1:,1:].tocsc())
X=frame[[f'x{i}' for i in range(1,k+1)]].to_numpy()
y=frame.y.to_numpy(); values=np.column_stack([y,X])
means=(D1.T@values)/n1[:,None]
rhs=D2.T@values-E.T@means
g2=np.zeros((len(n2),k+1));g2[1:]=factor.solve(rhs[1:])
# One direct residual correction exposes factorization/backward error separately.
normal_residual=rhs[1:]-S[1:,1:]@g2[1:]
correction=factor.solve(normal_residual);g2[1:]+=correction
g1=means-(E@g2)/n1[:,None]
within=values-D1@g1-D2@g2
beta,_,rank,_=lstsq(within[:,1:],within[:,0],cond=None,lapack_driver='gelsd')
assert rank==k
residual=within[:,0]-within[:,1:]@beta
left_effect=(g1[:,0]-g1[:,1:]@beta)[left]
right_effect=(g2[:,0]-g2[:,1:]@beta)[right]
left_effect-=left_effect.mean();right_effect-=right_effect.mean()
constant=float(np.mean(y-X@beta))
iterative=np.load(ROOT/'weak-oracle-values.npz')
result={'components':components,'finite_rank':int(rank),
        'normal_correction_maxabs':float(np.max(np.abs(correction))),
        'lsmr_residual_agreement_maxabs':float(np.max(np.abs(residual-iterative['residual']))),
        'lsqr_residual_agreement_maxabs':float(np.max(np.abs(residual-iterative['lsqr_residual']))),
        'projection_normal_maxabs':float(max(np.max(np.abs(D1.T@residual)),np.max(np.abs(D2.T@residual)))),
        'reconstruction_maxabs':float(np.max(np.abs(y-X@beta-constant-left_effect-right_effect-residual)))}
assert result['lsmr_residual_agreement_maxabs']<1e-10
assert result['lsqr_residual_agreement_maxabs']<1e-10
truth=pd.DataFrame(dict(obs_id=frame.obs_id,saved_e=residual,saved_g=left_effect,saved_h=right_effect))
truth.to_csv(ROOT/'weak-direct-oracle.csv',index=False,float_format='%.17g')
bread=np.linalg.inv(within[:,1:].T@within[:,1:])
score=within[:,1:]*residual[:,None]
df_r=n-(len(n1)+len(n2)-components)-k
covariance=(bread@(score.T@score)@bread)*(n/df_r)
pd.DataFrame(covariance).to_csv(ROOT/'weak-direct-covariance.csv',index=False,float_format='%.17g')
pd.DataFrame({'name':[f'x{i}' for i in range(1,k+1)],'value':beta}).to_csv(ROOT/'weak-direct-coefficients.csv',index=False,float_format='%.17g')
result['df_r']=int(df_r)
for arm in ['baseline','candidate']:
    run=ROOT/'runs'/f'control-weak_d3-s31415927-{arm}'
    saved=pd.read_csv(run/'saved_outputs.csv')
    result[arm]={}
    for col in ['saved_e','saved_g','saved_h']:
        error=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy())
        result[arm][col]={'maxabs':float(error.max()),'failing_rows':int(np.count_nonzero(error>limit)),
                          'largest_tolerance_ratio':float(np.max(error/limit))}
    slopes=pd.read_csv(run/'coefficients.csv').set_index('name').loc[[f'x{i}' for i in range(1,k+1)],'value'].to_numpy()
    full_cov=pd.read_csv(run/'V.csv').pivot(index='row',columns='column',values='value').to_numpy()
    assert np.allclose(slopes,beta,atol=1e-10,rtol=1e-7)
    assert np.allclose(full_cov[:k,:k],covariance,atol=1e-10,rtol=1e-6)
    assert json.loads((run/'result.json').read_text())['metadata']['df_r']==df_r
    result[arm]['finite_b_maxabs']=float(np.max(np.abs(slopes-beta)))
    result[arm]['finite_V_maxabs']=float(np.max(np.abs(full_cov[:k,:k]-covariance)))
(ROOT/'weak-direct-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2),flush=True)
