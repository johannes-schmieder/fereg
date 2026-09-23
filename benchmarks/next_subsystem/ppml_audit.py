"""Independent positive-outcome PPML slope/sample/mean diagnostics and small oracles."""
import argparse
import hashlib
import json

import numpy as np
import pandas as pd
from scipy import sparse
from scipy.special import gammaln, xlogy

from audit import audit, compare, distance, read
from oracles import oracle_sample
from oracle import solve


def design(frame, cell):
    blocks=[]
    for effect in cell['effects']:
        code,levels=pd.factorize(frame[effect],sort=True)
        F=sparse.csr_matrix((np.ones(len(frame)),(np.arange(len(frame)),code)),shape=(len(frame),len(levels)))
        blocks.append(F)
        blocks.extend(F.multiply(frame[name].to_numpy()[:,None]) for name in cell.get('slope_partitions',{}).get(effect,[]))
    return sparse.hstack(blocks,format='csr')


def means(path, sample, physical_n, ids):
    result=read(path/'result.json')
    ids=np.asarray(ids,dtype=np.int64)
    digest=hashlib.sha256(b''.join(int(x).to_bytes(8,'little') for x in ids)).hexdigest()
    if digest!=result['sample_sha256'] or digest!=sample['sha256']:
        raise ValueError('Saved means have a different estimation sample')
    mu=np.fromfile(path/'means.bin',dtype='<f8')
    if len(mu)==physical_n:mu=mu[ids-1]
    elif len(mu)!=len(ids):raise ValueError('Saved mean length differs from sample')
    if not (np.isfinite(mu)&(mu>0)).all():raise ValueError('Saved means must be positive and finite')
    return mu


def public_state(y,X,F,beta,mu,metadata):
    # Same 1e-8 stationarity and 1e-10/1e-12 fresh span thresholds as the
    # existing PPML public-state audit, extended to explicit slope columns.
    D=sparse.hstack([sparse.csr_matrix(X),F],format='csr')
    residual=y-mu
    scale=max(np.linalg.norm(residual/np.sqrt(mu)),np.linalg.norm(y/np.sqrt(mu)))
    moments=np.asarray(D.T@residual).ravel()
    squares=np.asarray(D.power(2).T@mu).ravel()
    score=float(np.max(np.abs(moments)/(np.sqrt(squares)*scale)))
    target=np.log(mu)-X@beta
    span_residual=target-F@solve(F,target)
    span_scale=max(1.,float(np.linalg.norm(target)))
    relative=float(np.linalg.norm(span_residual)/span_scale)
    norm=np.sqrt(np.asarray(F.power(2).sum(axis=0)).ravel())
    normal=float(np.max(np.abs(np.asarray(F.T@span_residual).ravel())/norm)/span_scale)
    ll=float(np.sum(y*np.log(mu)-mu-gammaln(y+1)))
    deviance=float(2*np.sum(xlogy(y,y/mu)-y+mu))
    replay={'ll':distance([ll],[metadata.get('ll',metadata.get('loglik'))],1e-8),
            'deviance':distance([deviance],[metadata['deviance']],1e-8)}
    return dict(score_max=score,stationarity_pass=bool(np.isfinite(score) and score<=1e-8),
                fresh_span_relative=relative,fresh_span_normal_max=normal,
                model_space='feasible' if relative<=1e-10 and normal<=1e-12 else 'indeterminate',
                log_likelihood=ll,deviance=deviance,replay=replay,
                passed=bool(score<=1e-8 and relative<=1e-10 and normal<=1e-12 and all(x['passed'] for x in replay.values())))


def explicit_poisson(y,X,F):
    if len(y)>7680 or F.shape[1]>1200:
        raise ValueError('Explicit PPML oracle exceeds bounded rank design')
    rank=int(np.linalg.matrix_rank(F.toarray()))
    n,k=X.shape
    if n<=rank+k:raise ValueError('PPML oracle requires positive residual DoF')
    D=sparse.hstack([sparse.csr_matrix(X),F],format='csr')
    eta=np.full(n,np.log(y.mean()))
    for iteration in range(100):
        mu=np.exp(eta);root=np.sqrt(mu)
        increment=solve(D.multiply(root[:,None]),(y-mu)/root)
        direction=D@increment;objective=np.sum(mu-y*eta);step=1.
        while step>2**-30:
            trial=eta+step*direction
            if np.isfinite(trial).all() and np.max(trial)<700 and np.sum(np.exp(trial)-y*trial)<=objective+1e-11*max(1,abs(objective)):break
            step*=.5
        if step<=2**-30:raise ValueError('Independent PPML line search failed')
        eta=trial;mu=np.exp(eta)
        scores=np.asarray(D.T@(y-mu)).ravel()
        scales=np.maximum(1,np.asarray(abs(D).T@(y+mu)).ravel())
        if np.max(abs(scores)/scales)<1e-11:break
    else:raise ValueError('Independent PPML iteration cap')
    beta=solve(D,eta)[:k];root=np.sqrt(mu)
    WF=F.multiply(root[:,None])
    Z=np.column_stack([X[:,j]-F@solve(WF,root*X[:,j]) for j in range(k)])
    if np.linalg.matrix_rank(Z)!=k:raise ValueError('Finite oracle design is rank deficient')
    bread=np.linalg.inv(Z.T@(mu[:,None]*Z))
    meat=Z.T@((y-mu)[:,None]**2*Z)
    V=bread@meat@bread*n/(n-1)
    return dict(beta=beta,V=V,mu=mu,fe_rank=rank,df_r=n-rank-k,iterations=iteration+1,
                ll=float(np.sum(y*eta-mu-gammaln(y+1))),
                deviance=float(2*np.sum(xlogy(y,y/mu)-y+mu)))


def check(root, independent=False):
    plan=read(root/'plan.json');progress=read(root/'progress.json');records=[]
    for index,cell in enumerate(plan['cells']):
        if not plan.get('ppml_surface') or cell['outputs']!='mu':raise ValueError('Expected PPML mean surface')
        full=pd.read_stata(root/f'data/c{index}-full.dta',convert_categoricals=False)
        frame,sample=oracle_sample(full,cell);F=design(frame,cell)
        X=frame[[f'x{j}' for j in range(1,cell['k']+1)]].to_numpy();y=frame.y.to_numpy()
        expected=explicit_poisson(y,X,F) if independent else None
        attempts=[x for x in progress if x['case']==index];states={};independent_checks={};pairs=[];output={}
        for attempt in attempts:
            path=root/attempt['result'];key=attempt['result'];result=read(path/'result.json')
            if result['status']!='pass':
                states[key]=dict(passed=False,status=result['status'])
                if expected:independent_checks[key]=dict(passed=False,status=result['status'])
                continue
            names=[f'x{j}' for j in range(1,cell['k']+1)]
            beta=pd.read_csv(path/'coefficients.csv').set_index('name').loc[names].value.to_numpy()
            mu=means(path,sample,len(full),frame.obs_id.to_numpy());output[key]=mu
            states[key]=public_state(y,X,F,beta,mu,result['metadata'])
            if expected:
                V=pd.read_csv(path/'covariance.csv').pivot(index='row',columns='column',values='value').loc[names,names].to_numpy()
                c=dict(coefficients=distance(beta,expected['beta'],1e-7),covariance=distance(V,expected['V'],1e-6),means=distance(mu,expected['mu'],1e-7,1e-9))
                c.update({k:distance([result['metadata'].get(k,result['metadata'].get('loglik'))],[expected[k]],1e-8) for k in ['ll','deviance']})
                independent_checks[key]=dict(checks=c,passed=all(v['passed'] for v in c.values()))
        for rep in range(plan['repeats']):
            by_variant={a['variant']:a for a in attempts if a['repetition']==rep};base=by_variant[plan['variants'][0]]
            for v,a in by_variant.items():
                if a is base:continue
                c=compare(root/base['result'],root/a['result'],'ppml')
                if base['result'] in output and a['result'] in output:
                    c['means']=distance(output[base['result']],output[a['result']],1e-7,1e-9);c['passed'] &= c['means']['passed']
                pairs.append(dict(repetition=rep,variant=v,checks=c))
        oracle_info={k:v for k,v in expected.items() if k not in ['beta','V','mu']} if expected else None
        records.append(dict(cell=cell,independent_sample=sample,public_states=states,pairs=pairs,oracles=independent_checks,oracle_info=oracle_info))
    return dict(scope='Positive unweighted numeric PPML, robust N/(N-1) sandwich and saved means; unresolved public states remain unranked',records=records,descriptive=audit(root))


if __name__=='__main__':
    from pathlib import Path
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);p.add_argument('output',type=Path);p.add_argument('--independent',action='store_true');a=p.parse_args()
    with a.output.open('x') as f:json.dump(check(a.root,a.independent),f,indent=2,allow_nan=False);f.write('\n')
