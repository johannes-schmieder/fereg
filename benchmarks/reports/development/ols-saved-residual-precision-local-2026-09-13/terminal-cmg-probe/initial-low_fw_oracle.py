import os
from pathlib import Path
import json,hashlib
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import splu,lsqr,lsmr
from scipy.sparse.csgraph import connected_components
from scipy.linalg import lstsq
root=Path(__file__).resolve().parent;req=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='low_fw');p=Path(req['data']);assert hashlib.sha256(p.read_bytes()).hexdigest()==req['data_sha256'];f=pd.read_stata(p,convert_categoricals=False);k=req['cell']['k'];names=[f'x{i}' for i in range(1,k+1)];assert not f[['y',*names,'g1','g2','w','c1']].isna().any().any();w=f.w.to_numpy();assert np.all(w>0) and np.array_equal(w,w.astype(int));n=len(f);N=int(w.sum());rows=np.arange(n);X=f[names].to_numpy();y=f.y.to_numpy();left=np.unique(f.g1,return_inverse=True)[1];right=np.unique(f.g2,return_inverse=True)[1];D1=sp.csr_matrix((np.ones(n),(rows,left)));D2=sp.csr_matrix((np.ones(n),(rows,right)));W=sp.diags(w);n1=np.asarray(D1.T@w).ravel();n2=np.asarray(D2.T@w).ravel();assert np.min(n1)>1 and np.min(n2)>1;E=D1.T@W@D2;S=sp.diags(n2)-E.T@sp.diags(1/n1)@E;components,labels=connected_components(S,directed=False);keep=np.ones(len(n2),dtype=bool)
for c in range(components):keep[np.flatnonzero(labels==c)[0]]=False
G=S[keep][:,keep].tocsc();factor=splu(G);v=np.column_stack([y,X]);means=(D1.T@(w[:,None]*v))/n1[:,None];rhs=D2.T@(w[:,None]*v)-E.T@means;g2=np.zeros((len(n2),k+1));g2[keep]=factor.solve(rhs[keep]);g2[keep]+=factor.solve(rhs[keep]-G@g2[keep]);g1=means-(E@g2)/n1[:,None];within=v-D1@g1-D2@g2;Z=within[:,1:];Zw=np.sqrt(w)[:,None]*Z;beta,_,rank,_=lstsq(Zw,np.sqrt(w)*within[:,0],cond=1e-12,lapack_driver='gelsd');assert rank==k;residual=within[:,0]-Z@beta;constant=float(np.average(y-X@beta,weights=w));a=(g1[:,0]-g1[:,1:]@beta)[left]-constant;b=(g2[:,0]-g2[:,1:]@beta)[right]
for c in range(components):
 selected=labels[right]==c;mean=np.average(b[selected],weights=w[selected]);b[selected]-=mean;a[selected]+=mean
# Literal frequency replication supplies the independent full-design oracle.
A=sp.hstack([sp.csr_matrix(X),D1,D2],format='csr');expanded=np.repeat(rows,w.astype(int));Af=A[expanded];yf=y[expanded];norms=np.sqrt(np.asarray(Af.multiply(Af).sum(axis=0)).ravel());scaled=Af@sp.diags(1/norms);iterative={}
for method in ['lsmr','lsqr']:
 solved=lsmr(scaled,yf,atol=2e-15,btol=2e-15,conlim=1e14,maxiter=20000) if method=='lsmr' else lsqr(scaled,yf,atol=2e-15,btol=2e-15,conlim=1e14,iter_lim=20000)
 assert solved[1] in [1,2,4,5];other=y-A@(solved[0]/norms);error=float(np.max(np.abs(other-residual)));assert error<1e-10;iterative[method]={'iterations':int(solved[2]),'stop':int(solved[1]),'frequency_expanded_rows':N,'residual_agreement_maxabs':error}
cluster=np.unique(f.c1,return_inverse=True)[1];groups=int(cluster.max()+1);nested=[]
for codes in [left,right]:
 low=np.full(codes.max()+1,np.iinfo(np.int64).max);high=np.full(codes.max()+1,-1);np.minimum.at(low,codes,cluster);np.maximum.at(high,codes,cluster);nested.append(bool(np.array_equal(low,high)))
fe_rank=len(n1)+len(n2)-components;df_a=fe_rank-sum(l for l,yes in zip([len(n1),len(n2)],nested) if yes);df_correction=N-k-df_a-int(any(nested));q=groups/(groups-1)*(N-1)/df_correction
_,s,vt=np.linalg.svd(Zw,full_matrices=False);bread=(vt.T/(s*s))@vt;psi=((w*residual)[:,None]*Z)@bread;psi=np.column_stack([psi,w*residual/N-psi@np.average(X,axis=0,weights=w)]);cluster_scores=np.zeros((groups,k+1));np.add.at(cluster_scores,cluster,psi);V=q*(cluster_scores.T@cluster_scores);coeff=np.r_[beta,constant];truth=pd.DataFrame(dict(obs_id=f.obs_id,saved_e=residual,saved_g=a,saved_h=b));truth.to_csv(root/'low-fw-direct-oracle.csv',index=False,float_format='%.17g');pd.DataFrame(V).to_csv(root/'low-fw-direct-covariance.csv',index=False,float_format='%.17g');pd.DataFrame({'value':coeff}).to_csv(root/'low-fw-direct-coefficients.csv',index=False,float_format='%.17g');result={'data_sha256':req['data_sha256'],'rows':n,'effective_N':N,'components':int(components),'rank':int(rank),'FE_rank':fe_rank,'nested':nested,'df_a':df_a,'df_r':groups-1,'df_fit':N-k-fe_rank,'df_correction':df_correction,'clusters':groups,'finite_sample_correction':q,'iterative':iterative}
for arm in ['baseline','candidate']:
 path=root/'cost-runs'/('low_fw-'+arm);saved=pd.read_csv(path/'saved_outputs.csv');assert np.array_equal(saved.obs_id,truth.obs_id);out={}
 for col in ['saved_e','saved_g','saved_h']:
  error=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());out[col]={'maxabs':float(error.max()),'failing_rows':int(np.count_nonzero(error>limit)),'max_tolerance_ratio':float(np.max(error/limit))}
 def mat(name):return pd.read_csv(path/(name+'.csv')).pivot(index='row',columns='column',values='value').to_numpy()
 ab=mat('b').reshape(-1);av=mat('V');assert np.allclose(ab,coeff,atol=1e-10,rtol=1e-7) and np.allclose(av,V,atol=1e-10,rtol=1e-6);out['b_error']=float(np.max(np.abs(ab-coeff)));out['V_error']=float(np.max(np.abs(av-V)));m=json.loads((path/'result.json').read_text())['metadata']
 for key,val in [('N',N),('N_full',n),('rank',rank),('df_r',groups-1),('df_a',df_a),('fereg_df_fit',N-k-fe_rank),('fereg_df_correction',df_correction)]:assert m[key]==val,(key,m[key],val)
 result[arm]=out
(root/'low-fw-direct-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True);assert all(result['candidate'][c]['failing_rows']==0 for c in ['saved_e','saved_g','saved_h']);print('LOW_FW_ORACLE_PASS',flush=True)
