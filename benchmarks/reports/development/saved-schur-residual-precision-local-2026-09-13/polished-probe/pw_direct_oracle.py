import os
"""Independent final-sample weighted dummy/SVD and two-way CGM oracle."""
from pathlib import Path
import argparse,hashlib,json
import numpy as np
import pandas as pd
import scipy.sparse as sp
from scipy.sparse.linalg import splu,lsmr,lsqr
from scipy.sparse.csgraph import connected_components
from scipy.linalg import lstsq
ROOT=Path(__file__).parent;parser=argparse.ArgumentParser();parser.add_argument('--seed',type=int,required=True);args=parser.parse_args();seed=args.seed;name='pw_partial'
request=next(x for x in json.loads((ROOT/'primary-requests.json').read_text()) if x['name']==name and x['seed']==seed)
path=Path(request['data']);assert hashlib.sha256(path.read_bytes()).hexdigest()==request['data_sha256']
original=pd.read_stata(path,convert_categoricals=False);k=request['cell']['k'];names=[f'x{i}' for i in range(1,k+1)]
frame=original.dropna(subset=['y',*names,'g1','g2','w','c1','c2']).copy();frame=frame[frame.w>0]
initial=len(frame);rounds=0
while True:
 keep=np.ones(len(frame),dtype=bool)
 for col in ['g1','g2']:
  _,codes,counts=np.unique(frame[col],return_inverse=True,return_counts=True);keep &= counts[codes]>1
 if keep.all():break
 frame=frame.loc[keep].copy();rounds+=1
n=len(frame);rows=np.arange(n);X=frame[names].to_numpy();y=frame.y.to_numpy();w=frame.w.to_numpy();sqrtw=np.sqrt(w)
_,left=np.unique(frame.g1,return_inverse=True);_,right=np.unique(frame.g2,return_inverse=True)
D1=sp.csr_matrix((np.ones(n),(rows,left)));D2=sp.csr_matrix((np.ones(n),(rows,right)));W=sp.diags(w)
n1=np.asarray(D1.T@w).ravel();n2=np.asarray(D2.T@w).ravel();E=D1.T@W@D2;S=sp.diags(n2)-E.T@sp.diags(1/n1)@E
components,labels=connected_components(S,directed=False);keep=np.ones(len(n2),dtype=bool)
for component in range(components):keep[np.flatnonzero(labels==component)[0]]=False
grounded=S[keep][:,keep].tocsc();factor=splu(grounded)
values=np.column_stack([y,X]);means=(D1.T@(w[:,None]*values))/n1[:,None];rhs=D2.T@(w[:,None]*values)-E.T@means
g2=np.zeros((len(n2),k+1));g2[keep]=factor.solve(rhs[keep]);correction=factor.solve(rhs[keep]-grounded@g2[keep]);g2[keep]+=correction
g1=means-(E@g2)/n1[:,None];within=values-D1@g1-D2@g2;Z=within[:,1:];Zw=sqrtw[:,None]*Z
beta,_,rank,_=lstsq(Zw,sqrtw*within[:,0],cond=1e-12,lapack_driver='gelsd');assert rank==k
residual=within[:,0]-Z@beta;constant=float(np.average(y-X@beta,weights=w))
left_effect=(g1[:,0]-g1[:,1:]@beta)[left]-constant;right_effect=(g2[:,0]-g2[:,1:]@beta)[right]
for component in range(components):
 selected=labels[right]==component;center=np.average(right_effect[selected],weights=w[selected]);right_effect[selected]-=center;left_effect[selected]+=center
print('Direct weighted FWL/SVD complete',n,rank,components,flush=True)
# The second family solves the full explicit design, independently of the reduced FE system.
A=sp.hstack([sp.csr_matrix(X),D1,D2],format='csr');Aw=sp.diags(sqrtw)@A;norms=np.sqrt(np.asarray(Aw.multiply(Aw).sum(axis=0)).ravel());scaled=Aw@sp.diags(1/norms)
iterative={}
for method in ['lsmr','lsqr']:
 if method=='lsmr':solution=lsmr(scaled,sqrtw*y,atol=2e-15,btol=2e-15,conlim=1e14,maxiter=20000)
 else:solution=lsqr(scaled,sqrtw*y,atol=2e-15,btol=2e-15,conlim=1e14,iter_lim=20000)
 assert solution[1] in [1,2,4,5]
 other=(sqrtw*y-scaled@solution[0])/sqrtw;iterative[method]={'stop':int(solution[1]),'iterations':int(solution[2]),'residual_agreement_maxabs':float(np.max(np.abs(residual-other)))}
 assert iterative[method]['residual_agreement_maxabs']<1e-10,iterative[method]
 print(method,iterative[method],flush=True)
truth=pd.DataFrame(dict(obs_id=frame.obs_id,saved_e=residual,saved_g=left_effect,saved_h=right_effect));prefix=f'pw-s{seed}-direct';truth.to_csv(ROOT/(prefix+'-oracle.csv'),index=False,float_format='%.17g')
clusters=[np.unique(frame[col],return_inverse=True)[1] for col in ['c1','c2']]
nested=[]
for fe in [left,right]:
 found=False
 for cluster in clusters:
  low=np.full(fe.max()+1,np.iinfo(np.int64).max,dtype=np.int64);high=np.full(fe.max()+1,-1,dtype=np.int64)
  np.minimum.at(low,fe,cluster);np.maximum.at(high,fe,cluster);found |= bool(np.array_equal(low,high))
 nested.append(found)
fe_rank=len(n1)+len(n2)-components;nested_levels=sum(levels for levels,is_nested in zip([len(n1),len(n2)],nested) if is_nested)
df_a=fe_rank-nested_levels;df_fit=n-k-fe_rank;df_correction=n-k-df_a-int(any(nested));groups=[int(c.max()+1) for c in clusters];df_r=min(groups)-1
_,singular,vt=np.linalg.svd(Zw,full_matrices=False);bread=(vt.T/(singular*singular))@vt;meanX=np.average(X,axis=0,weights=w)
psi=(w*residual)[:,None]*Z@bread;constant_psi=w*residual/w.sum()-psi@meanX;psi=np.column_stack([psi,constant_psi]);raw=np.zeros((k+1,k+1));subset_counts=[]
for colset,sign in [([0],1),([1],1),([0,1],-1)]:
 _,codes=np.unique(np.column_stack([clusters[c] for c in colset]),axis=0,return_inverse=True);sums=np.zeros((int(codes.max())+1,k+1));np.add.at(sums,codes,psi);raw+=sign*(sums.T@sums);subset_counts.append(len(sums))
q=min(groups)/(min(groups)-1)*(n-1)/df_correction;raw*=q;raw=(raw+raw.T)/2;eigen,vectors=np.linalg.eigh(raw);V=(vectors*np.maximum(eigen,0))@vectors.T;b=np.r_[beta,constant]
pd.DataFrame(V).to_csv(ROOT/(prefix+'-covariance.csv'),index=False,float_format='%.17g');pd.DataFrame({'value':b}).to_csv(ROOT/(prefix+'-coefficients.csv'),index=False,float_format='%.17g')
result={'seed':seed,'data_sha256':request['data_sha256'],'initial_complete_rows':initial,'singleton_rounds':rounds,'rows':n,'components':int(components),'finite_rank':int(rank),'weight_sum':float(w.sum()),'FE_levels':[len(n1),len(n2)],'nested':nested,'df_a':df_a,'df_fit':df_fit,'df_r':df_r,'df_correction':df_correction,'cluster_groups':groups,'subset_groups':subset_counts,'CGM_common_correction':q,'raw_negative_eigenvalues':int(np.count_nonzero(eigen<0)),'normal_correction_maxabs':float(np.max(np.abs(correction))),'iterative':iterative,'weighted_projection_maxabs':float(max(np.max(np.abs(D1.T@(w*residual))),np.max(np.abs(D2.T@(w*residual))))),'reconstruction_maxabs':float(np.max(np.abs(y-X@beta-constant-left_effect-right_effect-residual)))}
for arm in ['baseline','candidate']:
 run=ROOT/'runs'/f'primary-{name}-s{seed}-{arm}';saved=pd.read_csv(run/'saved_outputs.csv');assert np.array_equal(saved.obs_id,original.obs_id)
 for col in ['saved_e','saved_g']:saved[col]=pd.to_numeric(saved[col],errors='coerce')
 assert np.array_equal(saved.obs_id[saved.saved_e.notna()],frame.obs_id)
 assert np.array_equal(saved.saved_e.isna(),saved.saved_g.isna());saved=saved[saved.saved_e.notna()];out={}
 for col in ['saved_e','saved_g']:
  error=np.abs(saved[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy())
  out[col]={'maxabs':float(error.max()),'failing_rows':int(np.count_nonzero(error>limit)),'max_tolerance_ratio':float(np.max(error/limit))}
 def matrix(filename):return pd.read_csv(run/filename).pivot(index='row',columns='column',values='value').to_numpy()
 actual_b=matrix('b.csv').reshape(-1);actual_V=matrix('V.csv');out['full_b_maxabs']=float(np.max(np.abs(actual_b-b)));out['full_V_maxabs']=float(np.max(np.abs(actual_V-V)))
 assert np.allclose(actual_b,b,atol=1e-10,rtol=1e-7)
 assert np.allclose(actual_V,V,atol=1e-10,rtol=1e-6),(arm,out['full_V_maxabs'])
 meta=json.loads((run/'result.json').read_text())['metadata']
 for key,value in [('N',n),('rank',rank),('df_r',df_r),('df_a',df_a),('fereg_df_correction',df_correction),('fereg_df_fit',df_fit)]:assert meta[key]==value,(key,meta[key],value)
 result[arm]=out
(ROOT/(prefix+'-results.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
