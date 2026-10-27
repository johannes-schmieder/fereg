"""Small explicit-design independent OLS/Poisson and sandwich witnesses."""
from pathlib import Path
import numpy as np
import pandas as pd
from scipy.linalg import qr
from campaign import close_values, POLICY


def oracle(frame, cell):
    n=len(frame);k=cell.get('regressors',16)
    if n>1200:raise ValueError('explicit oracle exceeds its declared bound')
    X=frame[[f'x{i}' for i in range(1,k+1)]].to_numpy()
    F=np.column_stack([pd.get_dummies(frame[f'g{j}'],dtype=float).to_numpy()
                       for j in range(1,cell['effects']+1)])
    Q,R,piv=qr(F,mode='economic',pivoting=True)
    rank=int(np.sum(np.abs(np.diag(R))>1e-10*np.max(np.abs(np.diag(R)))))
    D=np.column_stack([X,Q[:,:rank]])
    y=frame.y.to_numpy()
    if cell['model']=='ols':
        theta=np.linalg.lstsq(D,y,rcond=None)[0];fitted=D@theta
        H=D.T@D
    else:
        theta=np.linalg.lstsq(D,np.full(n,np.log(y.mean())),rcond=None)[0]
        for iteration in range(80):
            eta=D@theta;fitted=np.exp(eta)
            score=D.T@(y-fitted);H=D.T@(fitted[:,None]*D)
            step=np.linalg.solve(H,score)
            old=np.sum(fitted-y*eta);length=1.
            while length>2**-25:
                trial=D@(theta+length*step)
                if np.max(trial)<500 and np.sum(np.exp(trial)-y*trial)<=old+1e-12*max(1,abs(old)):break
                length*=.5
            if length<=2**-25:raise ValueError('independent Newton line search exhausted')
            theta+=length*step
            fitted=np.exp(D@theta)
            scale=np.maximum(1,abs(D).T@(y+fitted))
            if np.max(abs(D.T@(y-fitted))/scale)<1e-12:break
        else:raise ValueError('independent Newton exhausted')
        H=D.T@(fitted[:,None]*D)
    bread=np.linalg.inv(H);residual=y-fitted
    if cell['vce']=='iid': V=bread*(residual@residual)/(n-k-rank)
    else:
        scores=D*residual[:,None]
        if cell['vce']=='cluster':
            codes,levels=pd.factorize(frame.cluster,sort=True);G=len(levels)
            sums=np.zeros((G,D.shape[1]));np.add.at(sums,codes,scores)
            meat=sums.T@sums
            if cell['model']=='ppml': correction=G/(G-1)
            else:
                # Worker FE is nested; each remaining categorical partition
                # contributes L-1 to the declared pairwise DoF convention.
                absorbed=sum(frame[f'g{j}'].nunique()-1 for j in range(2,cell['effects']+1))
                correction=(n-1)/(n-k-absorbed-1)*G/(G-1)
        else:
            meat=scores.T@scores
            correction=n/(n-1) if cell['model']=='ppml' else n/(n-k-rank)
        V=bread@meat@bread*correction
    return dict(beta=theta[:k].tolist(),covariance=V[:k,:k].tolist(),fitted=fitted,rank=rank,
                residual_score=float(np.max(abs(D.T@residual)/np.maximum(1,abs(D).T@(abs(y)+abs(fitted))))))


def validate_smoke(result):
    rows=[];expected={}
    for cell in result['cells']:
        record=next((r for r in result['records'] if r['cell']==cell['id']),None)
        if record is None:continue
        path=Path(record['directory']).parents[2]/'data'/f"{cell['model']}-{cell['design']}-n{cell['rows']}.dta"
        expected[cell['id']]=oracle(pd.read_stata(path,convert_categoricals=False),cell)
    for record in result['records']:
        cell=next(c for c in result['cells'] if c['id']==record['cell'])
        checks={}
        if record['status']=='ok':
            e=expected[cell['id']];a=record['capture']
            checks['coefficients']=close_values(a['beta'],e['beta'],POLICY['coefficient_rtol'],POLICY['coefficient_atol'])
            checks['covariance']=close_values(a['covariance'],e['covariance'],POLICY['vce_rtol'],POLICY['vce_atol'])
            p=np.load(Path(record['directory'])/'fitted.npz')['fitted']
            checks['fitted']=close_values(p,e['fitted'],POLICY['fitted_rtol'],POLICY['fitted_atol'])
            checks['sample']=a['retained_all'] and a['N']==cell['rows']
            if cell['model']=='ppml':checks['absorbed_rank']=a['absorbed_dof']==e['rank']
        # Informational: a covariance that matches up to one degrees-of-freedom scalar.
        scale=None
        if checks.get('covariance') is False:
            from campaign import agreement
            scale=agreement(dict(covariance=e['covariance']),a,['covariance'])['covariance_scale']
        rows.append(dict(cell=cell['id'],arm=record['arm'],checks=checks,covariance_scale=scale,
                         **{'pass':bool(checks) and all(checks.values())}))
    return rows
