import os
from pathlib import Path
import json,hashlib
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.csgraph import connected_components
from scipy.sparse.linalg import lsqr,lsmr
from scipy.linalg import lstsq
root=Path(__file__).resolve().parent;req=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='three_fe_saved');p=Path(req['data']);assert hashlib.sha256(p.read_bytes()).hexdigest()==req['data_sha256']
f=pd.read_stata(p,convert_categoricals=False);n=len(f);rows=np.arange(n);k=req['cell']['k'];codes=[np.unique(f[g],return_inverse=True)[1] for g in ['g1','g2','g3']];D=[sp.csr_matrix((np.ones(n),(rows,c))) for c in codes];left=D[0];rest=sp.hstack(D[1:],format='csr');counts=np.asarray(left.sum(axis=0)).ravel();E=left.T@rest;S=(rest.T@rest-E.T@sp.diags(1/counts)@E).toarray();values=np.column_stack([f.y,f[[f'x{i}' for i in range(1,k+1)]]]);means=(left.T@values)/counts[:,None];rhs=rest.T@values-E.T@means
v,s,vt=np.linalg.svd(S);rankS=int(np.count_nonzero(s>s[0]*1e-12));coef=(vt[:rankS].T/s[:rankS])@(v[:,:rankS].T@rhs);coef+=(vt[:rankS].T/s[:rankS])@(v[:,:rankS].T@(rhs-S@coef));g1=means-E@coef/counts[:,None];within=values-left@g1-rest@coef;X=values[:,1:];y=values[:,0];Z=within[:,1:];beta,_,rank,_=lstsq(Z,within[:,0],cond=1e-12,lapack_driver='gelsd');assert rank==k;residual=within[:,0]-Z@beta;constant=float(np.mean(y-X@beta));truth_g=(g1[:,0]-g1[:,1:]@beta)[codes[0]]
# The first two effects have separate components. Global three-FE centering
# does not identify their component-specific transfer constants. Verify the
# eliminated system's entire nullspace before comparing identifiable g1 values.
adj=sp.bmat([[None,left.T@D[1]],[D[1].T@left,None]],format='csr');components,labels=connected_components(adj,directed=False);obs_component=labels[codes[0]];totals=np.bincount(obs_component)
def center(x):return x-(np.bincount(obs_component,weights=x)/totals)[obs_component]
null_g1=-(E@vt[rankS:].T)/counts[:,None];null_centered=np.column_stack([center(null_g1[codes[0],j]) for j in range(null_g1.shape[1])]);assert np.max(np.abs(null_centered))<1e-10
truth_g=center(truth_g)
A=sp.hstack([sp.csr_matrix(X),*D],format='csr');norms=np.sqrt(np.asarray(A.multiply(A).sum(axis=0)).ravel());scaled=A@sp.diags(1/norms);iterative={}
for method in ['lsmr','lsqr']:
 solve=lsmr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,maxiter=20000) if method=='lsmr' else lsqr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,iter_lim=20000)
 assert solve[1] in [1,2,4,5];other=y-scaled@solve[0];err=float(np.max(np.abs(residual-other)));assert err<1e-10;iterative[method]={'iterations':int(solve[2]),'stop':int(solve[1]),'residual_agreement_maxabs':err}
_,sv,vtZ=np.linalg.svd(Z,full_matrices=False);bread=(vtZ.T/(sv*sv))@vtZ;psi=(residual[:,None]*Z)@bread;psi=np.column_stack([psi,residual/n-psi@X.mean(axis=0)]);df_r=n-k-(len(counts)+rankS);V=(n/df_r)*(psi.T@psi);b=np.r_[beta,constant]
truth=pd.DataFrame(dict(obs_id=f.obs_id,saved_e=residual,saved_g=truth_g,component=obs_component));truth.to_csv(root/'three-fe-direct-oracle.csv',index=False,float_format='%.17g');pd.DataFrame(V).to_csv(root/'three-fe-direct-covariance.csv',index=False,float_format='%.17g');pd.DataFrame({'value':b}).to_csv(root/'three-fe-direct-coefficients.csv',index=False,float_format='%.17g')
result={'input_sha256':req['data_sha256'],'n':n,'rank':int(rank),'FE_rank':int(len(counts)+rankS),'df_r':df_r,'pair_components':int(components),'reduced_nullity':int(len(s)-rankS),'nullspace_centered_g1_maxabs':float(np.max(np.abs(null_centered))),'iterative':iterative,'saved_g_scope':'identified within-first-pair-component contrasts; global individual FE gauge is nonunique'}
for arm in ['baseline','candidate']:
 path=root/'cost-runs'/f'three_fe_saved-{arm}';actual=pd.read_csv(path/'saved_outputs.csv');assert np.array_equal(actual.obs_id,truth.obs_id);out={}
 for col in ['saved_e','saved_g']:
  a=actual[col].to_numpy();a=center(a) if col=='saved_g' else a;target=truth[col].to_numpy();e=np.abs(a-target);limit=1e-9+1e-7*np.abs(target);out[col]={'maxabs':float(e.max()),'failing_rows':int(np.count_nonzero(e>limit)),'max_tolerance_ratio':float(np.max(e/limit))}
 def matrix(name):return pd.read_csv(path/(name+'.csv')).pivot(index='row',columns='column',values='value').to_numpy()
 ab=matrix('b').reshape(-1);av=matrix('V');assert np.allclose(ab,b,atol=1e-10,rtol=1e-7);assert np.allclose(av,V,atol=1e-10,rtol=1e-6);out['b_maxabs']=float(np.max(np.abs(ab-b)));out['V_maxabs']=float(np.max(np.abs(av-V)));out['global_saved_g_mean']=float(actual.saved_g.mean());meta=json.loads((path/'result.json').read_text())['metadata'];assert meta['N']==n and meta['rank']==rank and meta['df_r']==df_r and meta['df_a']==len(counts)+rankS;result[arm]=out
(root/'three-fe-direct-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
assert all(result['candidate'][x]['failing_rows']==0 for x in ['saved_e','saved_g'])
print('THREE_FE_INDEPENDENT_ORACLE_PASS',flush=True)
