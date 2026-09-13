"""Independent one-FE profile likelihood and two-way slope covariance oracle."""
from pathlib import Path
import hashlib,json
import numpy as np
import pandas as pd
from ppml_one_fe_compare import numeric
from common import write_json,sha256

root=Path(__file__).parent
k=257
data=root/'psd-data/pw_257-n7680-s104730.dta'
path=root/'runs/psd-functional-n7680-candidate'
df=pd.read_stata(data,convert_categoricals=False)
required=['y','w','off','g1','c1','c2']+[f'x{j}' for j in range(1,k+1)]
active=np.isfinite(df[required].to_numpy()).all(axis=1)&(df.w.to_numpy()>0)
# One categorical FE: missingness precedes singleton and all-zero group removal.
while True:
    current=df.loc[active]
    groups=current.groupby('g1',sort=True).agg(count=('y','size'),outcome=('y','sum'))
    keep=groups.index[(groups['count']>1)&(groups['outcome']>0)]
    updated=active&df.g1.isin(keep).to_numpy()
    if np.array_equal(updated,active):break
    active=updated
sample=df.loc[active]
receipt=json.loads((path/'result.json').read_text())
h=hashlib.sha256()
for value in sample.obs_id.to_numpy():h.update(int(value).to_bytes(8,'little'))
assert h.hexdigest()==receipt['sample_sha256']
x=sample[[f'x{j}' for j in range(1,k+1)]].to_numpy(dtype=float)
y=sample.y.to_numpy();off=sample.off.to_numpy();q=sample.w.to_numpy();q=q*(len(q)/q.sum())
_,g=np.unique(sample.g1.to_numpy(),return_inverse=True);ng=g.max()+1
Y=np.bincount(g,weights=q*y,minlength=ng)
assert np.all(Y>0)

def frame(beta,derivatives=True):
    eta=off+x@beta
    shift=np.full(ng,-np.inf);np.maximum.at(shift,g,eta)
    e=np.exp(eta-shift[g]);Z=np.bincount(g,weights=q*e,minlength=ng)
    mu=e*(Y/Z)[g]
    objective=np.dot(q*y,eta-shift[g])-np.dot(Y,np.log(Z))
    if not derivatives:return objective
    a=q*mu
    means=np.zeros((ng,k));np.add.at(means,g,a[:,None]*x);means/=Y[:,None]
    centered=x-means[g]
    score=centered.T@(q*(y-mu))
    hessian=centered.T@(a[:,None]*centered)
    return objective,mu,centered,score,hessian

beta=np.zeros(k);iterations=[]
for iteration in range(40):
    objective,mu,centered,score,hessian=frame(beta)
    step=np.linalg.solve(hessian,score)
    score_scale=np.maximum(1.0,np.sum(np.abs(centered)*np.abs(q*(y-mu))[:,None],axis=0))
    score_error=float(np.max(np.abs(score)/score_scale))
    iterations.append(dict(iteration=iteration,objective=float(objective),step_max=float(np.max(np.abs(step))),score_error=score_error))
    if np.max(np.abs(step))<=1e-11 and score_error<=1e-10:break
    factor=1.0
    for _ in range(40):
        trial=beta+factor*step
        if frame(trial,False)>=objective-64*np.finfo(float).eps*max(1.0,abs(objective)):break
        factor*=.5
    else:raise AssertionError('independent profile likelihood line search failed')
    beta=trial
else:raise AssertionError('independent profile likelihood did not converge')

scores=centered*(q*(y-mu))[:,None]
meat=np.zeros((k,k));groups=[]
for columns,sign in [(['c1'],1),(['c2'],1),(['c1','c2'],-1)]:
    labels=sample[columns].to_numpy();_,codes=np.unique(labels,axis=0,return_inverse=True)
    sums=np.zeros((codes.max()+1,k));np.add.at(sums,codes,scores)
    meat+=sign*(sums.T@sums);groups.append(len(sums))
bread=np.linalg.solve(hessian,np.eye(k));raw=(min(groups[:2])/(min(groups[:2])-1))*(bread@meat@bread)
raw=(raw+raw.T)/2
values,vectors=np.linalg.eigh(raw)
expected=(vectors*np.maximum(values,0.0))@vectors.T;expected=(expected+expected.T)/2
actual_b=numeric(path,'b')[0,:k];actual_v=numeric(path,'V')[:k,:k]
b_error=np.abs(beta-actual_b)/(1e-10+1e-7*np.abs(beta))
v_error=np.abs(expected-actual_v)/(1e-10+1e-6*np.abs(expected))
assert np.max(b_error)<=1.0,('coefficient',np.max(b_error))
assert np.max(v_error)<=1.0,('covariance',np.max(v_error))
assert np.array_equal(numeric(path,'feppml_cluster')[:2,2],groups[:2])
assert numeric(path,'feppml_certificate')[-1,0]==1
assert int(numeric(path,'feppml_fit')[1,0])==k
output=dict(oracle='NumPy profiled one-FE Poisson Newton solve, current-weight centered Hessian, independent tuple grouping, full two-way CGM and slope-block eigenvalue clipping',
    data_sha256=sha256(data),sample_sha256=h.hexdigest(),sample_rows=len(sample),fixed_effect_groups=int(ng),cluster_subset_groups=groups,
    iterations=iterations,coefficient_max_absolute_error=float(np.max(np.abs(beta-actual_b))),coefficient_max_registered_envelope=float(np.max(b_error)),
    covariance_max_absolute_error=float(np.max(np.abs(expected-actual_v))),covariance_max_registered_envelope=float(np.max(v_error)),
    raw_minimum_eigenvalue=float(values.min()),clipped_eigenvalues=int(np.count_nonzero(values<0)),
    covariance_rank=int(np.count_nonzero(values>1e-8*np.max(np.sum(np.abs(expected),axis=1)))),passed=True)
write_json(root/'profiled-oracle.json',output)
np.savez_compressed(root/'profiled-oracle.npz',beta=beta,covariance=expected,raw_covariance=raw,eigenvalues=values)
print(json.dumps(output,indent=2))
