"""Source-bound, non-headline adjudication of the three rejected pilot groups."""
import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

import numpy as np
import pandas as pd

HERE = Path(__file__).resolve().parent
CASES = [
    ('ols-three_fe_sparse-n61440-k10-t28-slopes_fast', 'reghdfe'),
    ('ppml-three_fe_sparse-n61440-k10-t28-slopes_fast', 'fixest_strict'),
    ('ppml-three_fe_sparse-n61440-k6-t4-slopes_low', 'fixest_strict'),
]
REFERENCE_LIMIT = 1800
ORACLE_LIMIT = 1800


def values(path, names):
    with (path/'coefficients.csv').open() as stream:
        b = {r['name']: float(r['value']) for r in csv.DictReader(stream)}
    with (path/'covariance.csv').open() as stream:
        V = {(r['row'],r['column']): float(r['value']) for r in csv.DictReader(stream)}
    return np.array([b[n] for n in names]), np.array([[V[n,m] for m in names] for n in names])


def compare(path, independent, cell, science):
    result = json.loads((path/'result.json').read_text())
    if result['status'] != 'pass':
        return dict(status='unavailable', arm_status=result['status'])
    b,V = values(path,independent['coefficient_names'])
    m = result['metadata'];field='rss' if cell['model']=='ols' else 'deviance'
    checks=dict(sample=result.get('sample_sha256')==independent['sample_sha256'],
                N=m.get('N')==independent['N'], converged=m.get('converged')==1,
                coefficients=bool(np.allclose(b,independent['coefficients'],rtol=science['coefficient_rtol'],atol=science['coefficient_atol'])),
                covariance=bool(np.allclose(V,independent['covariance'],rtol=science['vce_rtol'],atol=science['vce_atol'])),
                objective=math.isclose(m[field],independent[field],rel_tol=science['objective_rtol'],abs_tol=science['objective_atol']))
    if cell['model']=='ols' and result['arm'] in ('fereg','reghdfe'):
        checks['df_r'] = m.get('df_r')==independent['df_r']
    if result['arm']=='feppml':
        checks['model_rank'] = m.get('feppml_model_rank')==independent['model_rank']
    return dict(status='pass' if all(checks.values()) else 'accuracy_failure',checks=checks,
                max_coefficient_error=float(np.max(abs(b-independent['coefficients']))),
                max_covariance_error=float(np.max(abs(V-independent['covariance']))),
                objective_error=float(m[field]-independent[field]))


def main(root, output):
    # Import only the frozen measurement/generator code; this separate directory
    # never replaces any controller or estimator file in the benchmark run.
    sys.path.insert(0,str(root/'controller'))
    from common import manifest_for, read_json, sha256, write_json
    from generate import generate
    from provenance import verify
    from runner import measure
    identity = verify(root)
    if output.exists():
        raise ValueError('preserve an existing diagnostic attempt')
    output.mkdir(parents=True)
    source={p.name:sha256(p) for p in HERE.glob('*') if p.suffix in ('.py','.sh')}
    diagnostic_id=hashlib.sha256(json.dumps(source,sort_keys=True).encode()).hexdigest()
    manifest=manifest_for(root)
    plan=dict(schema='pilot-slope-adjudication-v1', parent_fingerprint=identity,
              source=source,diagnostic_id=diagnostic_id, cases=CASES,
              reference_command_limit=REFERENCE_LIMIT,oracle_wall_limit=ORACLE_LIMIT,
              purpose='Scientific adjudication only; no cross-node timing pairing',
              job_id=os.environ.get('JOB_ID'),slots=os.environ.get('NSLOTS'))
    write_json(output/'plan.json',plan)
    records=[]
    with tempfile.TemporaryDirectory() as tmp:
        scratch=Path(tmp)
        for name, arm in CASES:
            original=root/'results'/name/'warm-1'
            group=read_json(original/'GROUP.COMPLETE.json');cell=group['cell']
            directory=output/name;directory.mkdir()
            data=scratch/(name+'.dta')
            info=generate(cell,group['seed'],data,manifest)
            captured=read_json(original/'input.json')
            native='fereg' if cell['model']=='ols' else 'feppml'
            native_receipt=read_json(original/native/'result.json')
            if (info['column_sha256']!=captured['column_sha256'] or info['sha256']!=captured['sha256']
                    or info['sha256']!=native_receipt['input_sha256']):
                raise ValueError('regeneration must match every captured input byte')
            write_json(directory/'input.json',info)
            write_json(directory/'cell.json',cell)
            warm=scratch/(name+'-warm.dta')
            generate(dict(cell,n=manifest['measurement']['warmup_rows'],section='main'),group['seed'],warm,manifest)
            print('REFERENCE_BEGIN',name,arm,flush=True)
            measured=measure(root,cell,arm,'warm',data,warm,directory/arm,manifest,
                             identity='diagnostic-'+diagnostic_id,
                             limits=dict(startup=600,command=REFERENCE_LIMIT,extraction=600))
            print('REFERENCE_END',name,measured['status'],measured.get('seconds'),flush=True)
            # Oracle runs after the reference; it cannot contend with its fit.
            environment=os.environ.copy();environment.update(OPENBLAS_NUM_THREADS='4',OMP_NUM_THREADS='4',MKL_NUM_THREADS='4')
            command=[sys.executable,str(HERE/'run.py'),'--oracle',str(data),str(directory/'cell.json'),str(directory/'oracle.json')]
            print('ORACLE_BEGIN',name,flush=True)
            with (directory/'oracle.log').open('w') as log:
                try:
                    process=subprocess.run(command,env=environment,stdout=log,stderr=subprocess.STDOUT,timeout=ORACLE_LIMIT,check=False)
                    oracle_status='pass' if process.returncode==0 else 'failed'
                except subprocess.TimeoutExpired:
                    oracle_status='timeout'
            native='fereg' if cell['model']=='ols' else 'feppml'
            record=dict(cell=cell,original_group_sha256=sha256(original/'GROUP.COMPLETE.json'),
                        original_input_sha256=captured['sha256'],native_result_sha256=sha256(original/native/'result.json'),
                        native_output_hashes={p.name:sha256(p) for p in [original/native/'coefficients.csv',original/native/'covariance.csv']},
                        reference_output_hashes={p.name:sha256(p) for p in (directory/arm).glob('*.csv')},
                        reference_result_sha256=sha256(directory/arm/'result.json'),reference_status=measured['status'],
                        oracle_status=oracle_status)
            if oracle_status=='pass':
                independent=read_json(directory/'oracle.json')
                record.update(native=compare(original/native,independent,cell,manifest['science']),
                              reference=compare(directory/arm,independent,cell,manifest['science']),
                              oracle_sha256=sha256(directory/'oracle.json'))
            records.append(record)
            write_json(output/'progress.json',dict(plan=plan,records=records,status='running'))
            print('ORACLE_END',name,oracle_status,record.get('native'),record.get('reference'),flush=True)
    result=dict(plan=plan,records=records,status='complete',
                all_science_pass=all(r.get('native',{}).get('status')=='pass' and r.get('reference',{}).get('status')=='pass' for r in records))
    write_json(output/'result.json',result)
    print('PILOT_ADJUDICATION_COMPLETE',result['all_science_pass'],flush=True)


if __name__=='__main__':
    if sys.argv[1]=='--oracle':
        from explicit_slope_oracle import oracle
        data,cell,out=map(Path,sys.argv[2:])
        result=oracle(pd.read_stata(data,convert_categoricals=False),json.loads(cell.read_text()))
        out.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    else:
        parser=argparse.ArgumentParser();parser.add_argument('root',type=Path);parser.add_argument('output',type=Path)
        args=parser.parse_args();main(args.root,args.output)
