"""Separate full-coefficient/output comparisons and bounded explicit OLS oracles."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

import numpy as np
import pandas as pd

from audit import audit, compare, distance, read
from oracles import oracle_sample
from surface import columns
from oracle import dummy_matrix, solve


def outputs(path, n):
    values=np.fromfile(path/'outputs.bin',dtype='<f8')
    if values.size != 4*n:
        raise ValueError(f'Output shape mismatch: {values.size} versus {4*n}')
    return values.reshape(4,n)


def check(root, independent=False):
    plan=read(root/'plan.json'); progress=read(root/'progress.json')
    records=[]
    for index,cell in enumerate(plan['cells']):
        attempts=[x for x in progress if x['case']==index]
        pairs=[]
        for repetition in range(plan['repeats']):
            paths={x['variant']:root/x['result'] for x in attempts if x['repetition']==repetition}
            baseline=plan['variants'][0]
            for variant,path in paths.items():
                if variant==baseline:continue
                checks=compare(paths[baseline],path,'ols')
                if cell['outputs']=='all' and checks.get('sample') and checks.get('N'):
                    n=int(read(path/'result.json')['metadata']['N'])
                    checks['outputs']=distance(outputs(paths[baseline],n),outputs(path,n),1e-7,1e-9)
                    checks['passed'] &= checks['outputs']['passed']
                pairs.append(dict(repetition=repetition,variant=variant,checks=checks))
        oracles={}; sample=None
        if independent:
            frame=pd.read_stata(root/f'data/c{index}-full.dta',convert_categoricals=False)
            if len(frame)>7680:raise ValueError('Independent oracle is bounded to 7680 rows')
            frame,sample=oracle_sample(frame,cell)
            F,rank=dummy_matrix(frame,cell['effects']); X=columns(frame,cell)
            n,k=X.shape; y=frame.y.to_numpy()
            Z=np.column_stack([X[:,j]-F@solve(F,X[:,j]) for j in range(k)])
            y_within=y-F@solve(F,y)
            assert np.linalg.matrix_rank(Z)==k,'Finite surface is rank deficient'
            beta=np.linalg.lstsq(Z,y_within,rcond=None)[0]
            residual=y_within-Z@beta; rss=float(residual@residual)
            bread=np.linalg.inv(Z.T@Z)
            if cell['vce']=='robust':
                influence=(Z@bread)*residual[:,None]
                V=influence.T@influence*n/(n-rank-k)
            else:V=bread*rss/(n-rank-k)
            intercept=float(np.mean(y-X@beta))
            fitted=y-residual; finite=X@beta+intercept
            expected_outputs=np.array([fitted-finite,residual,finite,fitted])
            expected_names=cell['coefficient_names']
            for variant in plan['variants']:
                attempt=next(x for x in attempts if x['variant']==variant)
                path=root/attempt['result']; result=read(path/'result.json')
                if result['status']!='pass':
                    oracles[variant]=dict(passed=False,status=result['status']);continue
                b=pd.read_csv(path/'coefficients.csv').set_index('name').loc[expected_names].value
                v=pd.read_csv(path/'covariance.csv').pivot(index='row',columns='column',values='value').loc[expected_names,expected_names]
                checks=dict(sample=result['sample_sha256']==sample['sha256'],N=result['metadata']['N']==n,
                    coefficients=distance(b,beta,1e-7),covariance=distance(v,V,1e-6),rss=distance([result['metadata']['rss']],[rss],1e-8))
                if cell['outputs']=='all':checks['outputs']=distance(outputs(path,n),expected_outputs,1e-7,1e-9)
                oracles[variant]=dict(checks=checks,passed=all(x if isinstance(x,bool) else x['passed'] for x in checks.values()))
            sample.update(fe_rank=rank,finite_rank=k,df_r=n-rank-k,rss=rss)
        output_hashes={x['result']:hashlib.sha256((root/x['result']/'outputs.bin').read_bytes()).hexdigest()
                       for x in attempts if (root/x['result']/'outputs.bin').exists()}
        records.append(dict(cell=cell,pairs=pairs,independent_sample=sample,oracles=oracles,output_sha256=output_hashes))
    return dict(scope='Generated-term OLS development surface; all finite coefficients and requested outputs',
                records=records,descriptive=audit(root))


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('root',type=Path);p.add_argument('output',type=Path);p.add_argument('--independent',action='store_true')
    a=p.parse_args()
    with a.output.open('x') as f:json.dump(check(a.root,a.independent),f,indent=2,allow_nan=False);f.write('\n')
