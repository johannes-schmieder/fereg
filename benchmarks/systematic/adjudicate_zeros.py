"""Replay only the corrected independent oracle on captured zero-count inputs."""
import argparse
import csv
import math
import tempfile
from pathlib import Path

import numpy as np

from common import HERE, cell_id, manifest_for, read_json, sha256, write_json
from generate import generate
from light import smoke_groups
from oracle import oracle


def adjudicate(root, output):
    manifest=manifest_for(root)
    records=[]
    with tempfile.TemporaryDirectory() as folder:
        for group in smoke_groups(manifest):
            cell=group['cell']
            if cell['section']!='zeros':continue
            directory=root/'smoke-results'/cell_id(cell)/'warm-1'
            original=read_json(directory/'input.json')
            data=Path(folder)/(cell_id(cell)+'.dta')
            regenerated=generate(cell,manifest['seeds'][0],data,manifest)
            if regenerated['column_sha256'] != original['column_sha256'] or regenerated['sha256'] != original['sha256']:
                raise ValueError('Adjudication must use the exact captured input/runtime, not a cross-platform reconstruction')
            independent=oracle(data,cell)
            fit=directory/'feppml'
            result=read_json(fit/'result.json')
            with (fit/'coefficients.csv').open() as f:
                coefficients={r['name']:float(r['value']) for r in csv.DictReader(f)}
            with (fit/'covariance.csv').open() as f:
                covariance={(r['row'],r['column']):float(r['value']) for r in csv.DictReader(f)}
            names=independent['coefficient_names'];t=manifest['science']
            checks=dict(input=regenerated['sha256']==result['input_sha256'],
                        sample=independent['sample_sha256']==result['sample_sha256'],
                        coefficients=bool(np.allclose([coefficients[n] for n in names],independent['coefficients'],rtol=t['coefficient_rtol'],atol=t['coefficient_atol'])),
                        covariance=bool(np.allclose([[covariance[a,b] for b in names] for a in names],independent['covariance'],rtol=t['vce_rtol'],atol=t['vce_atol'])),
                        objective=math.isclose(result['metadata']['deviance'],independent['deviance'],rel_tol=t['objective_rtol'],abs_tol=t['objective_atol']))
            records.append(dict(cell=cell,checks=checks,oracle=independent,
                                original_input_sha256=original['sha256'],
                                captured_result_sha256=sha256(fit/'result.json'),
                                measurement_fingerprint=result['measurement_fingerprint']))
    receipt=dict(schema='systematic-zero-oracle-adjudication-v1',records=records,
                 sources={name:sha256(HERE/name) for name in ('oracle.py','generate.py','common.py','light.py','adjudicate_zeros.py')},
                 status='pass' if all(all(r['checks'].values()) for r in records) else 'failed')
    write_json(output,receipt)
    if receipt['status']!='pass':raise ValueError('Corrected independent zero oracle disagrees with captured native output')
    print('ZERO_ORACLE_ADJUDICATION_PASS',len(records),flush=True)
    return receipt


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('run_root',type=Path);parser.add_argument('output',type=Path)
    args=parser.parse_args();adjudicate(args.run_root,args.output)
