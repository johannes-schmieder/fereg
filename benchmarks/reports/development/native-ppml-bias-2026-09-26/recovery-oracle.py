"""Independent explicit-dummy Newton adjudication of the retained SCC failure."""
import argparse,hashlib,json
from pathlib import Path
import numpy as np
from scipy import sparse,linalg
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--work',type=Path,required=True)
p=parser.parse_args().work
raw=np.fromfile(p/'input.bin').reshape(-1,6)
y,g,h,x,z,exposure=raw.T
n=len(y);g=g.astype(int)-1;h=h.astype(int)-1
assert g.min()==h.min()==0 and g.max()==h.max()==255
rows=np.arange(n);keep=h>0
F=sparse.csr_matrix((np.ones(n+keep.sum()),(np.r_[rows,rows[keep]],np.r_[g,255+h[keep]])),shape=(n,511))
D=sparse.hstack([F,sparse.csr_matrix(np.c_[x,z])],format='csr')
offset=np.log(exposure);theta=np.zeros(513);theta[:256]=np.log(y.sum()/exposure.sum())
iterations=[]
for k in range(30):
 eta=offset+D@theta;mu=np.exp(eta);score=D.T@(y-mu)
 H=(D.T@D.multiply(mu[:,None])).toarray()
 step=linalg.solve(H,score,assume_a='pos')
 objective=np.sum(mu-y*eta);alpha=1.
 while alpha>1e-6:
  candidate=theta+alpha*step;ce=offset+D@candidate
  if np.sum(np.exp(ce)-y*ce)<=objective+1e-8: break
  alpha/=2
 assert alpha>1e-6
 theta=candidate
 iterations.append({'iteration':k+1,'max_step':float(np.max(np.abs(step))),'score':float(np.max(np.abs(score))/y.sum()),'alpha':alpha})
 if np.max(np.abs(step))<1e-12: break
else: raise AssertionError('independent Newton did not converge')
eta=offset+D@theta;mu=np.exp(eta)
report={'input_sha256':hashlib.sha256((p/'input.dta').read_bytes()).hexdigest(),'method':'Independent sparse explicit-dummy design, dense positive-definite Newton solve from constant start; no native outputs used as starts','iterations':iterations,'beta':theta[-2:].tolist(),'comparisons':{}}
for name in ['ordinary-v06','saved-v07','ordinary-v07','saved-t8-v07']:
 out=np.fromfile(p/(name+'.bin')).reshape(-1,4)
 weights=out[:,0];FH=(F.T@F.multiply(weights[:,None])).toarray()
 targets={'canonical_absorbed':out[:,3],'accepted_eta_minus_xb':out[:,1]-out[:,2]}
 spans={}
 for label,target in targets.items():
  coef=linalg.solve(FH,F.T@(weights*target),assume_a='pos')
  err=float(np.max(np.abs(target-F@coef)))
  spans[label]={'max_abs':err,'relative_infinity':err/float(np.max(np.abs(target)))}
 report['comparisons'][name]={'eta_max_abs':float(np.max(np.abs(out[:,1]-eta))),'mu_max_relative':float(np.max(np.abs(out[:,0]/mu-1))),'span':spans}
assert report['comparisons']['saved-v07']['eta_max_abs']<1e-10
assert report['comparisons']['saved-v07']['span']['canonical_absorbed']['relative_infinity']<1e-10
(p/'oracle-v07.json').write_text(json.dumps(report,indent=2)+'\n')
np.save(p/'oracle-eta.npy',eta)
print(json.dumps(report,indent=2))
