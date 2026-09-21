"""Bounded real-data explicit-dummy check, separate from performance fits."""
from pathlib import Path
import numpy as np
import pandas as pd
from common import read_json, write_json, sha256
from oracle import oracle
from measure import measure

ROOT = Path(__file__).resolve().parents[1]


def main():
    summary = []
    for model, arm in [('ols','fereg'),('ppml','feppml')]:
        frame = pd.read_stata(ROOT/f'work/{model}.dta',convert_categoricals=False)
        workers = np.random.default_rng(923).choice(frame.g1.unique(), 1500, replace=False)
        frame = frame[frame.g1.isin(workers)].copy()
        while True:
            keep = (frame.groupby('g1').g1.transform('size')>1) & (frame.groupby('g2').g2.transform('size')>1)
            if keep.all(): break
            frame = frame[keep].copy()
        frame = frame.reset_index(drop=True)
        path = ROOT/f'work/oracle-{model}.dta'
        frame.to_stata(path,write_index=False,version=118)
        write_json(path.with_suffix('.json'), {'n':len(frame),'columns':list(frame),
            'column_sha256': {c:__import__('hashlib').sha256(np.asarray(frame[c],dtype='<f8').tobytes()).hexdigest() for c in frame}})
        cell = {'model':model,'threads':1,'k':4,'n':len(frame),'effects':['g1','g2']}
        expected = oracle(path,cell)
        write_json(ROOT/f'results/oracle-{model}.json',expected)
        output = ROOT/f'work/validation/{model}'
        result = measure(ROOT,cell,arm,'cold',path,ROOT/f'work/warm-{model}.dta',output,
                         {'measurement':{'startup_timeout_seconds':600,'command_timeout_seconds':600,'rss_interval_seconds':.01}})
        if result['status']=='pass':
            observed = pd.read_csv(output/'coefficients.csv')
            b = observed.value.to_numpy(); se = observed.se.to_numpy()
            expected_b = np.asarray(expected['coefficients'])
            expected_se = np.sqrt(np.diag(expected['covariance']))
            checks = {'coefficients': bool(np.allclose(b,expected_b,rtol=1e-7,atol=1e-10)),
                      'standard_errors':bool(np.allclose(se,expected_se,rtol=1e-6,atol=1e-10)),
                      'sample_count':result['metadata'].get('N')==len(frame)}
            summary.append({'model':model,'N':len(frame),'checks':checks,
                            'max_absolute_b_error':float(np.max(abs(b-expected_b))),
                            'max_absolute_se_error':float(np.max(abs(se-expected_se)))})
        else:
            summary.append({'model':model,'status':result['status']})
        write_json(ROOT/f'results/validation-{model}.json',result)
    write_json(ROOT/'results/independent-validation.json',summary)
    print(summary,flush=True)


if __name__=='__main__': main()
