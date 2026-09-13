import os
"""Independent weighted dummy projection plus LAPACK SVD, checked by sparse LSQR/LSMR."""
from pathlib import Path
import hashlib,json
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import splu,lsmr,lsqr
from scipy.sparse.csgraph import connected_components
from scipy.linalg import lstsq
ROOT=Path(__file__).parent;name='rank_deficient_saved'
request=next(x for x in json.loads((ROOT/'control-requests.json').read_text()) if x['name']==name)
path=Path(request['data']);assert hashlib.sha256(path.read_bytes()).hexdigest()==request['data_sha256']
frame=pd.read_stata(path,convert_categoricals=False);n=len(frame);k=request['cell']['k'];rows=np.arange(n)
X=frame[[f'x{i}' for i in range(1,k+1)]].to_numpy();y=frame.y.to_numpy();w=frame.w.to_numpy();sqrtw=np.sqrt(w)
assert np.array_equal(X[:,0],X[:,1]) and np.all(w>0) and np.isfinite(X).all() and np.isfinite(y).all()
_,left=np.unique(frame.g1,return_inverse=True);_,right=np.unique(frame.g2,return_inverse=True)
D1=sp.csr_matrix((np.ones(n),(rows,left)));D2=sp.csr_matrix((np.ones(n),(rows,right)))
W=sp.diags(w);n1=np.asarray(D1.T@w).ravel();n2=np.asarray(D2.T@w).ravel();E=D1.T@W@D2
S=sp.diags(n2)-E.T@sp.diags(1/n1)@E
components,labels=connected_components(S,directed=False);keep=np.ones(len(n2),dtype=bool)
for component in range(components):keep[np.flatnonzero(labels==component)[0]]=False
grounded=S[keep][:,keep].tocsc();factor=splu(grounded)
values=np.column_stack([y,X]);means=(D1.T@(w[:,None]*values))/n1[:,None]
rhs=D2.T@(w[:,None]*values)-E.T@means
g2=np.zeros((len(n2),k+1));g2[keep]=factor.solve(rhs[keep]);correction=factor.solve(rhs[keep]-grounded@g2[keep]);g2[keep]+=correction
g1=means-(E@g2)/n1[:,None];within=values-D1@g1-D2@g2
all_beta,_,all_rank,singular=lstsq(sqrtw[:,None]*within[:,1:],sqrtw*within[:,0],cond=1e-12,lapack_driver='gelsd');assert all_rank==k-1
active=np.array([0,*range(2,k)]);Z=within[:,1:][:,active];Zw=sqrtw[:,None]*Z
beta_active,_,rank,_=lstsq(Zw,sqrtw*within[:,0],cond=1e-12,lapack_driver='gelsd');assert rank==k-1
beta=np.zeros(k);beta[active]=beta_active;residual=within[:,0]-Z@beta_active
assert np.max(np.abs(residual-(within[:,0]-within[:,1:]@all_beta)))<1e-12
left_effect=(g1[:,0]-g1[:,1:]@beta)[left];right_effect=(g2[:,0]-g2[:,1:]@beta)[right]
constant=float(np.average(y-X@beta,weights=w));left_effect-=constant
for component in range(components):
 selected=labels[right]==component;center=np.average(right_effect[selected],weights=w[selected]);right_effect[selected]-=center;left_effect[selected]+=center
A=sp.hstack([sp.csr_matrix(X),D1,D2],format='csr');Aw=sp.diags(sqrtw)@A
norms=np.sqrt(np.asarray(Aw.multiply(Aw).sum(axis=0)).ravel());scaled=Aw@sp.diags(1/norms)
iterative={}
for method in ['lsmr','lsqr']:
 if method=='lsmr':solution=lsmr(scaled,sqrtw*y,atol=2e-15,btol=2e-15,conlim=1e14,maxiter=20000)
 else:solution=lsqr(scaled,sqrtw*y,atol=2e-15,btol=2e-15,conlim=1e14,iter_lim=20000)
 assert solution[1] in [1,2,4,5]
 other=(sqrtw*y-scaled@solution[0])/sqrtw
 iterative[method]={'iterations':int(solution[2]),'stop':int(solution[1]),'residual_agreement_maxabs':float(np.max(np.abs(residual-other)))}
 assert iterative[method]['residual_agreement_maxabs']<1e-10,iterative[method]
truth=pd.DataFrame(dict(obs_id=frame.obs_id,saved_e=residual,saved_g=left_effect,saved_h=right_effect));truth.to_csv(ROOT/'rank-direct-oracle.csv',index=False,float_format='%.17g')
df_r=n-(len(n1)+len(n2)-components)-rank;sigma2=float(np.dot(w,residual**2)/df_r)
_,s,vt=np.linalg.svd(Zw,full_matrices=False);bread=(vt.T/(s*s))@vt;cov=sigma2*bread
meanX=np.average(X[:,active],axis=0,weights=w);V=np.zeros((k+1,k+1));V[np.ix_(active,active)]=cov
cross=-cov@meanX;V[active,k]=cross;V[k,active]=cross;V[k,k]=sigma2/w.sum()+meanX@cov@meanX
b=np.r_[beta,constant]
pd.DataFrame(V).to_csv(ROOT/'rank-direct-covariance.csv',index=False,float_format='%.17g');pd.DataFrame({'value':b}).to_csv(ROOT/'rank-direct-coefficients.csv',index=False,float_format='%.17g')
result={'components':int(components),'finite_rank':int(rank),'df_r':int(df_r),'data_sha256':request['data_sha256'],'weight_sum':float(w.sum()),'normal_correction_maxabs':float(np.max(np.abs(correction))),'iterative':iterative,'weighted_projection_maxabs':float(max(np.max(np.abs(D1.T@(w*residual))),np.max(np.abs(D2.T@(w*residual))))),'reconstruction_maxabs':float(np.max(np.abs(y-X@beta-constant-left_effect-right_effect-residual)))}
for arm in ['baseline','candidate']:
 run=ROOT/'runs'/f'control-{name}-s31415927-{arm}';saved=pd.read_csv(run/'saved_outputs.csv');assert np.array_equal(saved.obs_id,truth.obs_id)
 out={}
 for col in ['saved_e','saved_g','saved_h']:
  error=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy())
  failed=np.flatnonzero(error>limit)
  out[col]={'maxabs':float(error.max()),'failing_rows':len(failed),'largest_tolerance_ratio':float(np.max(error/limit)),'failed_obs_ids':[int(x) for x in truth.obs_id.iloc[failed]]}
 def matrix(filename):return pd.read_csv(run/filename).pivot(index='row',columns='column',values='value').to_numpy()
 observed_b=matrix('b.csv').reshape(-1);observed_V=matrix('V.csv')
 assert observed_b.shape==b.shape and observed_V.shape==V.shape
 assert np.allclose(observed_b,b,atol=1e-10,rtol=1e-7)
 assert np.allclose(observed_V,V,atol=1e-10,rtol=1e-6)
 assert json.loads((run/'result.json').read_text())['metadata']['df_r']==df_r
 out['full_b_maxabs']=float(np.max(np.abs(observed_b-b)));out['full_V_maxabs']=float(np.max(np.abs(observed_V-V)));result[arm]=out
(ROOT/'rank-direct-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
