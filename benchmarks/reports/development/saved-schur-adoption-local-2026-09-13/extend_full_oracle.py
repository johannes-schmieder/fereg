import os
"""Grounded explicit-dummy direct solve, with independently normalized FE outputs."""
from pathlib import Path
import json
import argparse
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import splu, lsmr, lsqr
from scipy.linalg import lstsq
from scipy.sparse.csgraph import connected_components

ROOT=Path(__file__).resolve().parent/'oracle-extension'
parser=argparse.ArgumentParser();parser.add_argument('name');args=parser.parse_args()
name=args.name
request=next(x for x in json.loads((ROOT/'control-requests.json').read_text()) if x['name']==name)
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
keep=np.ones(len(n2),dtype=bool)
for component in range(components):keep[np.flatnonzero(labels==component)[0]]=False
grounded=S[keep][:,keep].tocsc()
factor=splu(grounded)
X=frame[[f'x{i}' for i in range(1,k+1)]].to_numpy()
y=frame.y.to_numpy(); values=np.column_stack([y,X])
means=(D1.T@values)/n1[:,None]
rhs=D2.T@values-E.T@means
g2=np.zeros((len(n2),k+1));g2[keep]=factor.solve(rhs[keep])
# One direct residual correction exposes factorization/backward error separately.
normal_residual=rhs[keep]-grounded@g2[keep]
correction=factor.solve(normal_residual);g2[keep]+=correction
g1=means-(E@g2)/n1[:,None]
within=values-D1@g1-D2@g2
beta,_,rank,_=lstsq(within[:,1:],within[:,0],cond=None,lapack_driver='gelsd')
assert rank==k
residual=within[:,0]-within[:,1:]@beta
left_effect=(g1[:,0]-g1[:,1:]@beta)[left]
right_effect=(g2[:,0]-g2[:,1:]@beta)[right]
constant=float(np.mean(y-X@beta))
left_effect-=constant
for component in range(components):
    active=labels[right]==component
    center=right_effect[active].mean()
    right_effect[active]-=center
    left_effect[active]+=center
A=sp.hstack([sp.csr_matrix(X),D1,D2],format='csr')
norms=np.sqrt(np.asarray(A.multiply(A).sum(axis=0)).ravel())
scaled=A@sp.diags(1/norms)
a=lsmr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,maxiter=20000)
b=lsqr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,iter_lim=20000)
assert a[1] in [1,2,4,5] and b[1] in [1,2,4,5]
iterative={'residual':y-scaled@a[0], 'lsqr_residual':y-scaled@b[0]}
result={'components':components,'finite_rank':int(rank),
        'normal_correction_maxabs':float(np.max(np.abs(correction))),
        'lsmr_residual_agreement_maxabs':float(np.max(np.abs(residual-iterative['residual']))),
        'lsqr_residual_agreement_maxabs':float(np.max(np.abs(residual-iterative['lsqr_residual']))),
        'projection_normal_maxabs':float(max(np.max(np.abs(D1.T@residual)),np.max(np.abs(D2.T@residual)))),
        'reconstruction_maxabs':float(np.max(np.abs(y-X@beta-constant-left_effect-right_effect-residual)))}
assert result['lsmr_residual_agreement_maxabs']<1e-10
assert result['lsqr_residual_agreement_maxabs']<1e-10
truth=pd.DataFrame(dict(obs_id=frame.obs_id,saved_e=residual,saved_g=left_effect,saved_h=right_effect))
truth.to_csv(ROOT/f'{name}-direct-oracle.csv',index=False,float_format='%.17g')
bread=np.linalg.inv(within[:,1:].T@within[:,1:])
score=within[:,1:]*residual[:,None]
df_r=n-(len(n1)+len(n2)-components)-k
covariance=(bread@(score.T@score)@bread)*(n/df_r)
pd.DataFrame(covariance).to_csv(ROOT/f'{name}-direct-covariance.csv',index=False,float_format='%.17g')
pd.DataFrame({'name':[f'x{i}' for i in range(1,k+1)],'value':beta}).to_csv(ROOT/f'{name}-direct-coefficients.csv',index=False,float_format='%.17g')
result['df_r']=int(df_r)

# Differentiate b_cons = mean(y) - mean(X)' beta in the same fixed sample.
# Every observation contributes its raw residual to the mean response score.
linear=within[:,1:]@bread
influence=np.column_stack([linear, np.full(n,1/n)-linear@X.mean(axis=0)])
full_score=influence*residual[:,None]
full_covariance=full_score.T@full_score*(n/df_r)
full_beta=np.append(beta,constant)
assert np.allclose(full_covariance[:k,:k],covariance,atol=1e-14,rtol=1e-12)
# Independent response-direction fits use the grounded dummy projector and
# SVD finite solve rather than the inverse/influence multiplication.
result['response_direction_checks']=[]
for h in [np.sin(rows*.173), (left==0).astype(float)]:
 hm=(D1.T@h)/n1
 hr=D2.T@h-E.T@hm
 hc=np.zeros(len(n2));hc[keep]=factor.solve(hr[keep]);hl=hm-(E@hc)/n1
 hw=h-D1@hl-D2@hc
 db=lstsq(within[:,1:],hw,cond=None,lapack_driver='gelsd')[0]
 independent=np.append(db,float(np.mean(h-X@db)))
 error=float(np.max(np.abs(influence.T@h-independent)))
 result['response_direction_checks'].append(error);assert error<1e-10
truth.to_csv(ROOT/f'{name}-full-oracle.csv',index=False,float_format='%.17g')
pd.DataFrame(full_covariance).to_csv(ROOT/f'{name}-full-covariance.csv',index=False,float_format='%.17g')
pd.DataFrame({'name':[f'x{i}' for i in range(1,k+1)]+['_cons'],'value':full_beta}).to_csv(ROOT/f'{name}-full-coefficients.csv',index=False,float_format='%.17g')
result['full_dimensions']=len(full_beta)
result['input_sha256']=__import__('hashlib').sha256(Path(request['data']).read_bytes()).hexdigest()
(ROOT/f'{name}-full-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2),flush=True)
