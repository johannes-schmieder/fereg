import os
from pathlib import Path
import json,hashlib
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import splu,lsmr,lsqr
from scipy.sparse.csgraph import connected_components
from scipy.linalg import lstsq
root=Path(__file__).resolve().parent;req=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='explicit_cg_saved');p=Path(req['data']);assert hashlib.sha256(p.read_bytes()).hexdigest()==req['data_sha256'];f=pd.read_stata(p,convert_categoricals=False);n=len(f);rows=np.arange(n);k=req['cell']['k'];X=f[[f'x{i}' for i in range(1,k+1)]].to_numpy();y=f.y.to_numpy();left=np.unique(f.g1,return_inverse=True)[1];right=np.unique(f.g2,return_inverse=True)[1];D1=sp.csr_matrix((np.ones(n),(rows,left)));D2=sp.csr_matrix((np.ones(n),(rows,right)));n1=np.asarray(D1.sum(axis=0)).ravel();n2=np.asarray(D2.sum(axis=0)).ravel();E=D1.T@D2;S=sp.diags(n2)-E.T@sp.diags(1/n1)@E;components,labels=connected_components(S,directed=False);keep=np.ones(len(n2),dtype=bool)
for c in range(components):keep[np.flatnonzero(labels==c)[0]]=False
G=S[keep][:,keep].tocsc();factor=splu(G);v=np.column_stack([y,X]);means=(D1.T@v)/n1[:,None];rhs=D2.T@v-E.T@means;g2=np.zeros((len(n2),k+1));g2[keep]=factor.solve(rhs[keep]);g2[keep]+=factor.solve(rhs[keep]-G@g2[keep]);g1=means-(E@g2)/n1[:,None];within=v-D1@g1-D2@g2;Z=within[:,1:];beta,_,rank,_=lstsq(Z,within[:,0],cond=1e-12,lapack_driver='gelsd');assert rank==k;residual=within[:,0]-Z@beta;constant=float(np.mean(y-X@beta));a=(g1[:,0]-g1[:,1:]@beta)[left]-constant;b=(g2[:,0]-g2[:,1:]@beta)[right]
for c in range(components):
 selection=labels[right]==c;mu=b[selection].mean();b[selection]-=mu;a[selection]+=mu
A=sp.hstack([sp.csr_matrix(X),D1,D2],format='csr');norms=np.sqrt(np.asarray(A.multiply(A).sum(axis=0)).ravel());scaled=A@sp.diags(1/norms);iterative={}
for method in ['lsmr','lsqr']:
 solve=lsmr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,maxiter=20000) if method=='lsmr' else lsqr(scaled,y,atol=2e-15,btol=2e-15,conlim=1e14,iter_lim=20000)
 assert solve[1] in [1,2,4,5];err=float(np.max(np.abs(residual-(y-scaled@solve[0]))));assert err<1e-10;iterative[method]={'iterations':int(solve[2]),'stop':int(solve[1]),'residual_agreement_maxabs':err}
_,s,vt=np.linalg.svd(Z,full_matrices=False);bread=(vt.T/(s*s))@vt;meanX=X.mean(axis=0);B=np.block([[bread,(-bread@meanX)[:,None]],[(-meanX@bread)[None,:],np.array([[1/n+meanX@bread@meanX]])]]);df=n-k-len(n1)-len(n2)+components;V=(residual@residual/df)*B;coef=np.r_[beta,constant];truth=pd.DataFrame(dict(obs_id=f.obs_id,saved_e=residual,saved_g=a,saved_h=b));truth.to_csv(root/'auto-cmg-direct-oracle.csv',index=False,float_format='%.17g');pd.DataFrame(V).to_csv(root/'auto-cmg-direct-covariance.csv',index=False,float_format='%.17g');pd.DataFrame({'value':coef}).to_csv(root/'auto-cmg-direct-coefficients.csv',index=False,float_format='%.17g');result={'input_sha256':req['data_sha256'],'actual_solver_request':'auto; cost harness omitted intended explicit CG selector','components':int(components),'n':n,'rank':int(rank),'df_r':df,'iterative':iterative}
for arm in ['baseline','candidate']:
 run=root/'cost-runs'/('explicit_cg_saved-'+arm);saved=pd.read_csv(run/'saved_outputs.csv');assert np.array_equal(saved.obs_id,truth.obs_id);out={}
 for col in ['saved_e','saved_g','saved_h']:
  e=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());out[col]={'maxabs':float(e.max()),'failing_rows':int(np.count_nonzero(e>limit)),'max_tolerance_ratio':float(np.max(e/limit))}
 def matrix(name):return pd.read_csv(run/(name+'.csv')).pivot(index='row',columns='column',values='value').to_numpy()
 assert np.allclose(matrix('b').reshape(-1),coef,atol=1e-10,rtol=1e-7);assert np.allclose(matrix('V'),V,atol=1e-10,rtol=1e-6);out['b_error']=float(np.max(np.abs(matrix('b').reshape(-1)-coef)));out['V_error']=float(np.max(np.abs(matrix('V')-V)));meta=json.loads((run/'result.json').read_text())['metadata'];assert meta['df_r']==df and meta['rank']==rank and meta['N']==n;result[arm]=out
(root/'auto-cmg-direct-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True);assert all(result['candidate'][c]['failing_rows']==0 for c in ['saved_e','saved_g','saved_h']);print('AUTO_CMG_ORACLE_PASS',flush=True)
