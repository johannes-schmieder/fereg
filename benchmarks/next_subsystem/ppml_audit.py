"""Independent positive-outcome PPML slope/sample/mean diagnostics and small oracles."""
import argparse
import hashlib
import json
from itertools import combinations

import numpy as np
import pandas as pd
from scipy import sparse
from scipy.special import gammaln, xlogy

from audit import audit, compare, distance, read
from oracles import oracle_sample
from oracle import solve
from surface import columns


def design(frame, cell):
    blocks=[]
    for effect in cell['effects']:
        code,levels=pd.factorize(frame[effect],sort=True)
        F=sparse.csr_matrix((np.ones(len(frame)),(np.arange(len(frame)),code)),shape=(len(frame),len(levels)))
        blocks.append(F)
        blocks.extend(F.multiply(frame[name].to_numpy()[:,None]) for name in cell.get('slope_partitions',{}).get(effect,[]))
    return sparse.hstack(blocks,format='csr')


def saved_rows(path, filename, width, sample, physical_n, ids):
    result=read(path/'result.json')
    ids=np.asarray(ids,dtype=np.int64)
    digest=hashlib.sha256(b''.join(int(x).to_bytes(8,'little') for x in ids)).hexdigest()
    if digest!=result['sample_sha256'] or digest!=sample['sha256']:
        raise ValueError('Saved outputs have a different estimation sample')
    data=np.fromfile(path/filename,dtype='<f8')
    if len(data)==physical_n*width:data=data.reshape(width,physical_n).T[ids-1]
    elif len(data)==len(ids)*width:data=data.reshape(width,len(ids)).T
    else:raise ValueError('Saved output length differs from sample')
    if not np.isfinite(data).all():raise ValueError('Saved outputs must be finite')
    return data


def means(path, sample, physical_n, ids):
    mu=saved_rows(path,'means.bin',1,sample,physical_n,ids)[:,0]
    if not (np.isfinite(mu)&(mu>0)).all():raise ValueError('Saved means must be positive and finite')
    return mu


def output_state(values,y,X,beta,mu,frame,cell):
    xb,d,eta,residual=values[:,:4].T
    checks=dict(xb=distance(xb,X@beta,1e-7,1e-9),
                eta=distance(eta,np.log(mu),1e-7,1e-9),
                reconstruction=distance(xb+d,eta,1e-7,1e-9),
                residual=distance(residual,y-mu,1e-7,1e-9),
                channel_sum=distance(values[:,4:].sum(axis=1),d,1e-7,1e-9))
    for j,effect in enumerate(cell['effects']):
        code,_=pd.factorize(frame[effect],sort=True)
        # Each saved categorical channel must lie in its own partition's span.
        first=np.unique(code,return_index=True)[1]
        checks[effect+'_span']=distance(values[:,4+j],values[first,4+j][code],1e-7,1e-9)
    return dict(checks=checks,passed=all(x['passed'] for x in checks.values()))


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


def sandwich(bread,scores,clusters=None):
    n=len(scores)
    if clusters is None or clusters.shape[1]==0:
        return bread@(scores.T@scores)@bread*n/(n-1)
    counts=[len(np.unique(clusters[:,j])) for j in range(clusters.shape[1])]
    if min(counts)<2:raise ValueError('Independent cluster oracle needs at least two groups')
    meat=np.zeros_like(bread)
    for size in range(1,clusters.shape[1]+1):
        for subset in combinations(range(clusters.shape[1]),size):
            _,code=np.unique(clusters[:,subset],axis=0,return_inverse=True)
            grouped=np.zeros((code.max()+1,scores.shape[1]))
            np.add.at(grouped,code,scores)
            meat+=(-1)**(size+1)*(grouped.T@grouped)
    V=(bread@meat@bread)*min(counts)/(min(counts)-1)
    V=(V+V.T)/2
    eigenvalues,vectors=np.linalg.eigh(V)
    if eigenvalues[0]<0:V=(vectors*np.maximum(eigenvalues,0))@vectors.T
    return V


def explicit_poisson(y,X,F,clusters=None):
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
    V=sandwich(bread,(y-mu)[:,None]*Z,clusters)
    structural=n-rank-k
    reported=min(structural,min(len(np.unique(clusters[:,j])) for j in range(clusters.shape[1]))-1) if clusters is not None and clusters.shape[1] else structural
    return dict(beta=beta,V=V,mu=mu,fe_rank=rank,df_structural=structural,df_r=reported,iterations=iteration+1,
                ll=float(np.sum(y*eta-mu-gammaln(y+1))),
                deviance=float(2*np.sum(xlogy(y,y/mu)-y+mu)))


def check(root, independent=False):
    plan=read(root/'plan.json');progress=read(root/'progress.json');records=[]
    for index,cell in enumerate(plan['cells']):
        if not plan.get('ppml_surface') or cell['outputs'] not in ('mu','all'):raise ValueError('Expected PPML output surface')
        full=pd.read_stata(root/f'data/c{index}-full.dta',convert_categoricals=False)
        frame,sample=oracle_sample(full,cell);F=design(frame,cell)
        X=columns(frame,cell);y=frame.y.to_numpy()
        clusters=frame[cell.get('clusters',[])].to_numpy()
        expected=explicit_poisson(y,X,F,clusters) if independent else None
        attempts=[x for x in progress if x['case']==index];states={};independent_checks={};pairs=[];output={};channels={}
        for attempt in attempts:
            path=root/attempt['result'];key=attempt['result'];result=read(path/'result.json')
            if result['status']!='pass':
                states[key]=dict(passed=False,status=result['status'])
                if expected:independent_checks[key]=dict(passed=False,status=result['status'])
                continue
            names=cell.get('coefficient_names',[f'x{j}' for j in range(1,cell['k']+1)])
            beta=pd.read_csv(path/'coefficients.csv').set_index('name').loc[names].value.to_numpy()
            mu=means(path,sample,len(full),frame.obs_id.to_numpy());output[key]=mu
            states[key]=public_state(y,X,F,beta,mu,result['metadata'])
            if cell['outputs']=='all':
                values=saved_rows(path,'outputs.bin',4+len(cell['effects']),sample,len(full),frame.obs_id.to_numpy())
                channels[key]=values
                states[key]['outputs']=output_state(values,y,X,beta,mu,frame,cell)
                states[key]['passed'] &= states[key]['outputs']['passed']
            if expected:
                V=pd.read_csv(path/'covariance.csv').pivot(index='row',columns='column',values='value').loc[names,names].to_numpy()
                c=dict(coefficients=distance(beta,expected['beta'],1e-7),covariance=distance(V,expected['V'],1e-6),means=distance(mu,expected['mu'],1e-7,1e-9))
                if result['arm']!='fixest':
                    c['dof']=dict(passed=bool(result['metadata'].get('df_structural')==expected['df_structural']
                                             and result['metadata'].get('df_resid')==expected['df_r']))
                c.update({k:distance([result['metadata'].get(k,result['metadata'].get('loglik'))],[expected[k]],1e-8) for k in ['ll','deviance']})
                independent_checks[key]=dict(checks=c,passed=all(v['passed'] for v in c.values()))
        for rep in range(plan['repeats']):
            by_variant={a['variant']:a for a in attempts if a['repetition']==rep}
            base=by_variant.get(plan['variants'][0])
            for v in plan['variants'][1:]:
                a=by_variant.get(v)
                if base is None or a is None:
                    missing=[name for name in [plan['variants'][0],v] if name not in by_variant]
                    pairs.append(dict(repetition=rep,variant=v,
                                      checks=dict(passed=False,incomplete=True,missing_variants=missing)))
                    continue
                c=compare(root/base['result'],root/a['result'],'ppml')
                if base['result'] in output and a['result'] in output:
                    c['means']=distance(output[base['result']],output[a['result']],1e-7,1e-9);c['passed'] &= c['means']['passed']
                if base['result'] in channels and a['result'] in channels:
                    # Individual FE gauges may differ; identified contributions
                    # and each arm's independent reconstruction govern parity.
                    c['outputs']=distance(channels[base['result']][:,:4],channels[a['result']][:,:4],1e-7,1e-9)
                    c['passed'] &= c['outputs']['passed']
                pairs.append(dict(repetition=rep,variant=v,checks=c))
        oracle_info={k:v for k,v in expected.items() if k not in ['beta','V','mu']} if expected else None
        records.append(dict(cell=cell,independent_sample=sample,public_states=states,pairs=pairs,oracles=independent_checks,oracle_info=oracle_info))
    return dict(scope='Positive unweighted PPML, declared generated terms, robust N/(N-1) or common-min-G clustered sandwich and declared outputs; unresolved states remain unranked',records=records,descriptive=audit(root))


if __name__=='__main__':
    from pathlib import Path
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);p.add_argument('output',type=Path);p.add_argument('--independent',action='store_true');a=p.parse_args()
    with a.output.open('x') as f:json.dump(check(a.root,a.independent),f,indent=2,allow_nan=False);f.write('\n')
