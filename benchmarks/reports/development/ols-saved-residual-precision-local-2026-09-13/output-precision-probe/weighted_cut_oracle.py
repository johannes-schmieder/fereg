import os
"""80-digit FE reduction on the unchanged tiny-weight-bridge input."""
from pathlib import Path
import json,hashlib
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import lsmr,lsqr
from scipy.linalg import lstsq
import mpmath as mp
root=Path(__file__).resolve().parent;req=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='weighted_cut');p=Path(req['data']);assert hashlib.sha256(p.read_bytes()).hexdigest()==req['data_sha256'];f=pd.read_stata(p,convert_categoricals=False);n=len(f);k=req['cell']['k'];X=f[[f'x{i}' for i in range(1,k+1)]].to_numpy();y=f.y.to_numpy();w=f.w.to_numpy();v=np.column_stack([y,X]);left=np.unique(f.g1,return_inverse=True)[1];right=np.unique(f.g2,return_inverse=True)[1];n1=int(left.max()+1);n2=int(right.max()+1);mp.mp.dps=80;S=mp.matrix(n2);rhs=mp.matrix(n2,k+1);means=[];groups=[];counts=[];nw=[]
order=np.argsort(left,kind='stable');starts=np.r_[0,np.cumsum(np.bincount(left))]
for level in range(n1):
 rows=order[starts[level]:starts[level+1]];groups.append(rows);weights=[mp.mpf(float(w[r])) for r in rows];total=mp.fsum(weights);nw.append(total);values=[[mp.mpf(float(x)) for x in v[r]] for r in rows];mean=[mp.fsum(weights[t]*values[t][j] for t in range(len(rows)))/total for j in range(k+1)];means.append(mean);by={}
 for row,weight,value in zip(rows,weights,values):
  g=int(right[row]);by[g]=by.get(g,mp.mpf(0))+weight
  for j in range(k+1):rhs[g,j]+=weight*(value[j]-mean[j])
 counts.append(by);keys=sorted(by)
 for a,g in enumerate(keys):
  for h in keys[a+1:]:
   c=by[g]*by[h]/total;S[g,g]+=c;S[h,h]+=c;S[g,h]-=c;S[h,g]-=c
print('80-digit reduced operator assembled',n1,n2,flush=True)
G=S[1:,1:];R=rhs[1:,:];inv=G**-1;solution=inv*R;normal_error=max(abs(x) for x in G*solution-R);assert normal_error<mp.mpf('1e-55');g2=mp.matrix(n2,k+1)
for i in range(1,n2):
 for j in range(k+1):g2[i,j]=solution[i-1,j]
g1=[];within=np.empty_like(v)
for level,rows in enumerate(groups):
 a=[means[level][j]-mp.fsum(weight*g2[g,j] for g,weight in counts[level].items())/nw[level] for j in range(k+1)];g1.append(a)
 for row in rows:
  for j in range(k+1):within[row,j]=float(mp.mpf(float(v[row,j]))-a[j]-g2[int(right[row]),j])
Z=within[:,1:];Zw=np.sqrt(w)[:,None]*Z;beta,_,rank,singular=lstsq(Zw,np.sqrt(w)*within[:,0],cond=1e-12,lapack_driver='gelsd');assert rank==k;residual=within[:,0]-Z@beta;constant=float(np.average(y-X@beta,weights=w));betamp=[mp.mpf(float(x)) for x in beta];a=np.array([float(row[0]-mp.fsum(row[j+1]*betamp[j] for j in range(k))) for row in g1])[left]-constant;b=np.array([float(g2[i,0]-mp.fsum(g2[i,j+1]*betamp[j] for j in range(k))) for i in range(n2)])[right];center=np.average(b,weights=w);b-=center;a+=center
rows=np.arange(n);D1=sp.csr_matrix((np.ones(n),(rows,left)));D2=sp.csr_matrix((np.ones(n),(rows,right)));A=sp.hstack([sp.csr_matrix(X),D1,D2],format='csr');Aw=sp.diags(np.sqrt(w))@A;norms=np.sqrt(np.asarray(Aw.multiply(Aw).sum(axis=0)).ravel());scaled=Aw@sp.diags(1/norms);iterative={}
for method in ['lsmr','lsqr']:
 solve=lsmr(scaled,np.sqrt(w)*y,atol=1e-16,btol=1e-16,conlim=1e16,maxiter=20000) if method=='lsmr' else lsqr(scaled,np.sqrt(w)*y,atol=1e-16,btol=1e-16,conlim=1e16,iter_lim=20000)
 other=y-A@(solve[0]/norms);iterative[method]={'stop':int(solve[1]),'iterations':int(solve[2]),'residual_agreement_maxabs':float(np.max(np.abs(residual-other))),'weighted_residual_agreement_norm':float(np.linalg.norm(np.sqrt(w)*(residual-other)))};assert solve[1] in [1,2,4,5];print(method,iterative[method],flush=True)
_,s,vt=np.linalg.svd(Zw,full_matrices=False);bread=(vt.T/(s*s))@vt;meanX=np.average(X,axis=0,weights=w);B=np.block([[bread,(-bread@meanX)[:,None]],[(-meanX@bread)[None,:],np.array([[1/w.sum()+meanX@bread@meanX]])]]);df=n-k-n1-n2+1;V=(np.dot(w,residual*residual)/df)*B;coef=np.r_[beta,constant];truth=pd.DataFrame(dict(obs_id=f.obs_id,saved_e=residual,saved_g=a,saved_h=b));truth.to_csv(root/'weighted-cut-direct-oracle.csv',index=False,float_format='%.17g');pd.DataFrame(V).to_csv(root/'weighted-cut-direct-covariance.csv',index=False,float_format='%.17g');pd.DataFrame({'value':coef}).to_csv(root/'weighted-cut-direct-coefficients.csv',index=False,float_format='%.17g');result={'input_sha256':req['data_sha256'],'mp_decimal_digits':80,'mp_reduced_normal_maxabs':str(normal_error),'rows':n,'FE_levels':[n1,n2],'rank':int(rank),'df_r':df,'finite_condition':float(s.max()/s.min()),'iterative':iterative,'reconstruction_maxabs':float(np.max(np.abs(y-X@beta-constant-a-b-residual)))}
for arm in ['baseline','candidate']:
 run=root/'cost-runs'/('weighted_cut-'+arm);saved=pd.read_csv(run/'saved_outputs.csv');assert np.array_equal(saved.obs_id,truth.obs_id);out={}
 for col in ['saved_e','saved_g','saved_h']:
  error=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());out[col]={'maxabs':float(error.max()),'failing_rows':int(np.count_nonzero(error>limit)),'max_tolerance_ratio':float(np.max(error/limit))}
 def mat(name):return pd.read_csv(run/(name+'.csv')).pivot(index='row',columns='column',values='value').to_numpy()
 ab=mat('b').reshape(-1);av=mat('V');out['b_error']=float(np.max(np.abs(ab-coef)));out['V_error']=float(np.max(np.abs(av-V)));assert np.allclose(ab,coef,atol=1e-10,rtol=1e-7) and np.allclose(av,V,atol=1e-10,rtol=1e-6);meta=json.loads((run/'result.json').read_text())['metadata'];assert meta['df_r']==df and meta['rank']==rank and meta['N']==n;result[arm]=out
(root/'weighted-cut-direct-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True);assert all(result['candidate'][c]['failing_rows']==0 for c in ['saved_e','saved_g','saved_h']);print('WEIGHTED_CUT_ORACLE_PASS',flush=True)
