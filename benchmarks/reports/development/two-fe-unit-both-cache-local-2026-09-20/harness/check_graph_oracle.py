"""Validate the sparse-direct oracle against a grounded explicit-dummy SVD."""
from pathlib import Path
import sys,json,hashlib
import numpy as np,pandas as pd
from scipy.linalg import lstsq
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.graph_oracle import solve
assert len(sys.argv)==2, 'provide a new diagnostic output directory'
root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=False)
checks=[]
for disconnected in [False,True]:
 for kind in ['none','aw','fw']:
  for reverse in [False,True]:
   n=600;i=np.arange(n);rng=np.random.default_rng(104729)
   component=i%2 if disconnected else np.zeros(n,dtype=int)
   left=i//2%17+component*17;right=(i//34)%13+component*13
   X=rng.normal(size=(n,3));y=X@np.array([.3,-.1,.07])+np.sin(left)+np.cos(right)+rng.normal(size=n)
   w=rng.integers(1,7,n).astype(float) if kind=='fw' else np.exp(rng.normal(size=n))
   if kind=='none':w=np.ones(n)
   frame=pd.DataFrame(dict(g1=left,g2=right,y=y,w=w,x1=X[:,0],x2=X[:,1],x3=X[:,2]))
   if reverse:frame=frame.iloc[::-1].reset_index(drop=True)
   path=root/f'd{int(disconnected)}-{kind}-r{int(reverse)}.dta';frame.to_stata(path,write_index=False,version=118)
   raw={name:np.asarray(frame[name],dtype='<f8').tobytes() for name in frame}
   path.with_suffix('.bin').write_bytes(b''.join(raw.values()))
   path.with_suffix('.json').write_text(json.dumps(dict(n=n,k=3,columns=list(frame),components=2 if disconnected else 1,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),column_sha256={name:hashlib.sha256(value).hexdigest() for name,value in raw.items()})))
   actual=solve(path,kind)
   left_dummy=pd.get_dummies(frame.g1,dtype=float).to_numpy();right_dummy=pd.get_dummies(frame.g2,dtype=float).to_numpy()
   right_keep=np.ones(right_dummy.shape[1],dtype=bool);right_keep[0]=False
   if disconnected:right_keep[13]=False
   D=np.column_stack([left_dummy,right_dummy[:,right_keep]])
   weight=frame.w.to_numpy();weight=weight*n/weight.sum() if kind=='aw' else weight
   Z=np.column_stack([frame[['x1','x2','x3']].to_numpy(),D]);Y=frame.y.to_numpy();sqrt=np.sqrt(weight)
   b,_,rank,_=lstsq(Z*sqrt[:,None],Y*sqrt,lapack_driver='gelsd')
   residual=Y-Z@b
   covariance=np.linalg.pinv(Z.T@(Z*weight[:,None]),rcond=1e-13)[:3,:3]*(residual@(weight*residual))/(actual['n']-rank)
   np.testing.assert_allclose(actual['beta'],b[:3],rtol=1e-10,atol=1e-12)
   np.testing.assert_allclose(actual['covariance'],covariance,rtol=1e-9,atol=1e-12)
   np.testing.assert_allclose(actual['rss'],residual@(weight*residual),rtol=1e-12)
   assert actual['rank']==rank
   checks.append(dict(disconnected=disconnected,kind=kind,reversed=reverse,status='pass',rank=int(rank),coefficient_error=float(np.max(np.abs(actual['beta']-b[:3])))))
(root/'results.json').write_text(json.dumps(checks,indent=2)+'\n')
print('SPARSE_DIRECT_VS_DENSE_SVD_PASS',len(checks))
