"""Small explicit-design independent OLS/Poisson and sandwich witnesses."""
import hashlib
from pathlib import Path
import numpy as np
import pandas as pd
from scipy import sparse
from scipy.linalg import qr
from scipy.sparse.csgraph import connected_components
from campaign import close_values, POLICY, SEPARATION_DESIGNS


def separation(frame, cell):
    """Rows and controls a PPML fit cannot estimate, found without the packages.

    Rows in an all-zero group of any absorbed effect are separated, and so are
    the nonzero rows of a one-signed control that is nonzero only where the
    outcome is zero; such a control is then zero on the retained rows and is
    omitted. Dropping rows with a zero outcome never empties a positive group,
    so the order of these steps does not matter. Any other form of separation
    leaves the Newton witness below without convergence.
    """
    k=cell.get('regressors',16);y=frame.y.to_numpy()
    selected=np.ones(len(frame),dtype=bool)
    if cell['model']!='ppml' or cell['design'] not in SEPARATION_DESIGNS:return selected,[]
    for j in range(1,cell['effects']+1):
        code=frame[f'g{j}'].to_numpy(dtype=int)
        selected&=np.bincount(code,weights=y)[code]>0
    for j in range(1,k+1):
        x=frame[f'x{j}'].to_numpy()
        for sign in [1,-1]:
            if (sign*x[selected]>=0).all() and (y[selected&(sign*x>0)]==0).all():selected&=~(sign*x>0)
    return selected,[f'x{j}' for j in range(1,k+1) if (frame[f'x{j}'].to_numpy()[selected]==0).all()]


def oracle(frame, cell):
    if len(frame)>1200:raise ValueError('explicit oracle exceeds its declared bound')
    selected,omitted=separation(frame,cell)
    sample=hashlib.sha256(frame.obs_id.to_numpy(dtype='<i8')[selected].tobytes()).hexdigest()
    frame=frame[selected]
    names=[f'x{i}' for i in range(1,cell.get('regressors',16)+1) if f'x{i}' not in omitted]
    n=len(frame);k=len(names)
    X=frame[names].to_numpy()
    F=np.column_stack([pd.get_dummies(frame[f'g{j}'],dtype=float).to_numpy()
                       for j in range(1,cell['effects']+1)])
    Q,R,piv=qr(F,mode='economic',pivoting=True)
    rank=int(np.sum(np.abs(np.diag(R))>1e-10*np.max(np.abs(np.diag(R)))))
    # Reported counts find redundancies between the first effect and each
    # later one; this is the exact rank with two effects and an upper bound
    # with more, as in the gravity design's exporter, importer and pair effects.
    codes=[pd.factorize(frame[f'g{j}'])[0] for j in range(1,cell['effects']+1)]
    first=int(codes[0].max()+1);pairwise=first
    for code in codes[1:]:
        levels=int(code.max()+1)
        graph=sparse.coo_matrix((np.ones(n),(codes[0],code+first)),shape=(first+levels,)*2)
        pairwise+=levels-connected_components(graph,directed=False)[0]
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
                pairwise_rank=int(pairwise),names=names,selected=selected,sample_sha256=sample,
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
            checks['coefficients']=(a.get('names')==e['names']
                                    and close_values(a['beta'],e['beta'],POLICY['coefficient_rtol'],POLICY['coefficient_atol']))
            checks['covariance']=close_values(a['covariance'],e['covariance'],POLICY['vce_rtol'],POLICY['vce_atol'])
            p=np.load(Path(record['directory'])/'fitted.npz')['fitted']
            checks['fitted']=(bool(np.isnan(p[~e['selected']]).all())
                              and close_values(p[e['selected']],e['fitted'],POLICY['fitted_rtol'],POLICY['fitted_atol']))
            checks['sample']=a['N']==int(e['selected'].sum()) and a['sample_sha256']==e['sample_sha256']
            if cell['model']=='ppml':checks['absorbed_rank']=a['absorbed_dof']==e['pairwise_rank']
            checks={name:bool(value) for name,value in checks.items()}
        # Informational: a covariance that matches up to one degrees-of-freedom scalar.
        scale=None
        if checks.get('covariance') is False:
            from campaign import agreement
            scale=agreement(dict(covariance=e['covariance']),a,['covariance'])['covariance_scale']
        rows.append(dict(cell=cell['id'],arm=record['arm'],checks=checks,covariance_scale=scale,
                         **{'pass':bool(checks) and all(checks.values())}))
    return rows
