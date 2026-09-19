"""Small dense explicit-design startup oracle, independent of all estimators."""
from itertools import combinations
from pathlib import Path

import numpy as np
import pandas as pd
from scipy.special import xlogy

from common import write_json
from science import coefficients, design


def expected(data,c):
    f=pd.read_stata(data,convert_categoricals=False)
    if len(f)>768:
        raise ValueError("dense oracle is restricted to startup fixtures")
    X=f[[f"x{j}" for j in range(1,c["k"]+1)]].to_numpy()
    F=design(f,c).toarray()
    D=np.column_stack([X,F])
    y=f.y.to_numpy()
    rank=int(np.linalg.matrix_rank(D))
    if c["model"]=="ols":
        beta=np.linalg.lstsq(D,y,rcond=1e-12)[0]
        eta=D@beta
        mu=eta
        bread=np.linalg.pinv(D.T@D,rcond=1e-12)
    else:
        eta=np.full(len(f),np.log(y.mean()))
        for _ in range(200):
            mu=np.exp(eta)
            increment=np.linalg.lstsq(D*np.sqrt(mu)[:,None],(y-mu)/np.sqrt(mu),rcond=1e-12)[0]
            direction=D@increment
            current=float(np.sum(mu-y*eta))
            step=1.
            while step>2**-30:
                trial=eta+step*direction
                if np.max(trial)<700 and np.sum(np.exp(trial)-y*trial)<=current+1e-12*max(1,abs(current)):
                    break
                step*=.5
            eta=trial
            if np.max(np.abs(D.T@(y-np.exp(eta)))/np.maximum(1,np.abs(D).T@(y+np.exp(eta))))<1e-12:
                break
        else:
            raise ValueError("explicit Poisson oracle did not converge")
        mu=np.exp(eta)
        beta=np.linalg.lstsq(D,eta,rcond=1e-12)[0]
        bread=np.linalg.pinv(D.T@(mu[:,None]*D),rcond=1e-12)
    residual=y-mu
    if not c["clusters"] and c["model"]=="ols":
        V=bread*(residual@residual)/(len(f)-rank)
    else:
        scores=D*residual[:,None]
        if c["clusters"]:
            meat=np.zeros((D.shape[1],D.shape[1]))
            counts=[]
            for size in range(1,len(c["clusters"])+1):
                for names in combinations(c["clusters"],size):
                    codes=pd.factorize(pd.MultiIndex.from_frame(f[list(names)]),sort=True)[0]
                    sums=np.zeros((codes.max()+1,D.shape[1]))
                    np.add.at(sums,codes,scores)
                    meat+=(-1)**(size+1)*(sums.T@sums)
                    if size==1:
                        counts.append(len(sums))
            q=min(counts)/(min(counts)-1)
            if c["model"]=="ols":
                # Both absorbed dimensions are nested in declared clusters in
                # this startup fixture. The constant still costs one degree.
                q *= (len(f)-1)/(len(f)-c["k"]-1)
        else:
            meat=scores.T@scores
            q=len(f)/(len(f)-1)
        V=bread@meat@bread*q
    return dict(coefficients=beta[:c["k"]],covariance=V[:c["k"],:c["k"]],rank=rank,
                fitted=mu,objective=float(residual@residual) if c["model"]=="ols" else float(2*np.sum(xlogy(y,y/mu)-(y-mu))))


def check_fixture(data,c,output):
    output=Path(output)
    truth=expected(data,c)
    got=coefficients(output)
    names=[f"x{j}" for j in range(1,c["k"]+1)]
    b=np.array([got[x][0] for x in names])
    checks=dict(coefficients=bool(np.allclose(b,truth["coefficients"],rtol=1e-7,atol=1e-10)))
    if (output/"covariance.csv").exists():
        V=pd.read_csv(output/"covariance.csv").set_index(["row","column"]).value
        values=V.reindex([(x,y) for x in names for y in names]).to_numpy().reshape(c["k"],c["k"])
        checks["covariance"]=bool(np.allclose(values,truth["covariance"],rtol=1e-6,atol=1e-10))
    else:
        checks["SE_only"]=bool(np.allclose([got[x][1] for x in names],np.sqrt(np.diag(truth["covariance"])),rtol=1e-6,atol=1e-10))
    state=np.fromfile(output/"state.bin",dtype="<f8").reshape(4,-1)
    checks["sample"]=bool(np.array_equal(state[0],np.arange(1,c["n"]+1)))
    checks["prediction"]=bool(np.allclose(state[1],truth["fitted"],rtol=1e-7,atol=1e-8))
    result=dict(status="pass" if all(checks.values()) else "fail",checks=checks,max_b_error=float(np.max(np.abs(b-truth["coefficients"]))))
    write_json(output/"oracle.json",result)
    return result
