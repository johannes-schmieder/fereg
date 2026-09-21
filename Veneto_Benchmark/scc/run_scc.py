"""Isolated SCC comparison groups; one CPU setting and repetition per task."""
import argparse
import hashlib
import os
import random
import shutil
from pathlib import Path

import numpy as np
import pandas as pd

from common import read_json, write_json, sha256
from measure import measure
from oracle import oracle

HERE = Path(__file__).resolve().parent
ARMS = {'ols':['fereg','reghdfe','greg','areg','fixest','pyfixest'],
        'ppml':['feppml','ppmlhdfe','gglm','fixest','pyfixest']}
THREADS = [1,2,4,8,14,28]
CONTROLLER = ['run_scc.py','measure.py','common.py','stata_worker.do','python_worker.py','r_worker.R','oracle.py','job.sh']


def runtime_identity(root):
    inventory = read_json(root/'input/runtime-inventory.json')
    base = Path(inventory['runtime_root'])
    for name, expected in inventory['runtime'].items():
        if sha256(base/name) != expected:
            raise ValueError(f'Frozen SCC runtime changed: {name}')
    controller={name:sha256(HERE/name) for name in CONTROLLER}
    value={'runtime':inventory,'controller':controller}
    identity=hashlib.sha256(__import__('json').dumps(value,sort_keys=True).encode()).hexdigest()
    return identity,value


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('root',type=Path)
    parser.add_argument('--smoke',action='store_true')
    parser.add_argument('--task',type=int,default=1)
    args=parser.parse_args();root=args.root.resolve()
    assert int(os.environ['NSLOTS'])==28
    assert os.cpu_count()==28, 'Expected the registered 28-core node class'
    identity,value=runtime_identity(root)
    suite='smoke' if args.smoke else 'campaign'
    task=root/f'results/{suite}/task-{args.task:02d}'
    task.mkdir(parents=True,exist_ok=False)
    write_json(task/'runtime.json',value)
    if args.smoke:
        rep=0;cores=[1,28];phase='cold';prefix='synthetic'
    else:
        assert 1<=args.task<=24
        rep=(args.task-1)//6;cores=[THREADS[(args.task-1)%6]]
        phase='cold' if rep==0 else 'warm';prefix='veneto'
        assert (root/'results/smoke-accepted.json').exists(), 'Smoke acceptance required'
        assert (root/'input/data-authorization.json').exists(), 'Approved data storage required'
    manifest={'measurement':{'rss_interval_seconds':.01,'startup_timeout_seconds':600,
                             'command_timeout_seconds':120 if args.smoke else 2400},
              'threads':THREADS,'warm_repetitions':3,'cold_repetitions':1,'arms':ARMS,
              'platform':'BU SCC Linux E5-2680v4','task':args.task,'repetition':rep}
    write_json(task/'manifest.json',manifest)
    receipts=[];oracle_checks=[]
    for threads in cores:
        for model,arms in ARMS.items():
            data=root/f'input/{prefix}-{model}.dta'
            warm=root/f'input/synthetic-{model}.dta'
            info=read_json(data.with_suffix('.json'))
            cell={'model':model,'threads':threads,'k':4,'n':info['n'],'effects':['g1','g2']}
            order=arms.copy();random.Random(301+rep*29+threads).shuffle(order)
            expected=oracle(data,cell) if args.smoke else None
            if expected:write_json(task/f'oracle-{model}-t{threads}.json',expected)
            for arm in order:
                output=root/f'work/{suite}/task-{args.task:02d}/{model}-t{threads}-r{rep}-{arm}'
                print(output.name,flush=True)
                receipt=measure(root,cell,arm,phase,data,warm,output,manifest,identity)
                receipt['repetition']=rep
                cp=output/'coefficients.csv'
                if cp.exists():receipt['coefficients']=pd.read_csv(cp).to_dict('records')
                write_json(task/(output.name+'.json'),receipt)
                receipts.append(receipt)
                if args.smoke and arm in ['fereg','feppml']:
                    assert receipt['status']=='pass', f'{arm} smoke command failed'
                    coeff=pd.read_csv(cp)
                    passed=(np.allclose(coeff.value,expected['coefficients'],rtol=1e-7,atol=1e-10)
                            and np.allclose(coeff.se,np.sqrt(np.diag(expected['covariance'])),rtol=1e-6,atol=1e-10)
                            and receipt['metadata']['N']==info['n'])
                    oracle_checks.append({'model':model,'threads':threads,'pass':bool(passed)})
                    assert passed, f'{arm} independent smoke check failed'
                print(receipt['status'],receipt['seconds'],receipt['peak_rss_gib'],flush=True)
    write_json(task/'COMPLETE.json',{'calls':len(receipts),'oracle_checks':oracle_checks,
                                   'runtime_fingerprint':identity,'job_id':os.environ['JOB_ID'],
                                   'task_id':args.task,'suite':suite})
    print('VENETO_SCC_TASK_COMPLETE',flush=True)


if __name__=='__main__':main()
