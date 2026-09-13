import os
"""Materialize the prospective saved-output controls before any control timing."""
from pathlib import Path
import copy
import datetime
import hashlib
import json
import numpy as np
import pandas as pd
from preparation_compare import input_for
from run import MANIFEST

ROOT = Path(os.environ['SAVED_SCREEN_ROOT'])
SEED = 31415927
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
requests = []

def record(cell, data, warm, **extra):
    for path in [data, warm]:
        assert sha(path) == json.loads(path.with_suffix('.json').read_text())['sha256']
    request = dict(name=cell['name'], seed=SEED, cell=cell,
                   data=str(data), warm=str(warm), data_sha256=sha(data),
                   warm_sha256=sha(warm), **extra)
    requests.append(request)
    print(json.dumps({'prepared': cell['name'], 'recipe': cell}), flush=True)

for name in ['weighted_cut', 'weak_d3', 'disconnected_blocks', 'unbalanced',
             'small_aw', 'low_fw', 'complete_iid']:
    oldpath = Path(os.path.join(os.environ['CG_ENERGY_ROOT'], 'runs')) / f'control-{name}-s{SEED}-candidate/request.json'
    old = json.loads(oldpath.read_text())
    cell = copy.deepcopy(old['cell'])
    if name != 'complete_iid':
        cell['absorb'] = 'saved_g=g1' if cell['effects'] == ['g1'] else 'saved_g=g1 saved_h=g2'
        cell['fit_options'] = (cell.get('fit_options', '') + ' residuals(saved_e)').strip()
    record(cell, Path(old['data']), Path(old['warm']), reused_request=str(oldpath),
           changes_from_reused_request={key: value for key, value in cell.items()
                                        if old['cell'].get(key) != value})

specs = [
    dict(name='narrow_saved', design='strong_d3', n=30720, k=10, threads=4,
         effects=['g1','g2'], absorb='saved_g=g1 saved_h=g2', fit_options='residuals(saved_e)'),
    dict(name='slope', design='strong_d3', n=30720, k=16, threads=4,
         effects=['g1','g2'], weight='[aw=w]', vce='robust',
         absorb='saved_g=g1 g2#c.x1', fit_options='residuals(saved_e)'),
    dict(name='three_fe_saved', design='three_fe_sparse', n=30720, k=16, threads=4,
         effects=['g1','g2','g3'], vce='robust',
         absorb='saved_g=g1 g2 g3', fit_options='residuals(saved_e)'),
    dict(name='rank_deficient_saved', design='strong_d2', n=30720, k=50, threads=4,
         effects=['g1','g2'], weight='[aw=w]',
         absorb='saved_g=g1 saved_h=g2', fit_options='residuals(saved_e)'),
    dict(name='explicit_cg_saved', design='strong_d3', n=30720, k=16, threads=4,
         effects=['g1','g2'], absorb='saved_g=g1 saved_h=g2', fit_options='residuals(saved_e)'),
]
assert 'three_fe_sparse' in MANIFEST['designs']

def duplicate_column(original):
    target = original.with_name(original.stem + '-x2-equals-x1.dta')
    assert not target.exists(), target
    frame = pd.read_stata(original, convert_categoricals=False)
    frame['x2'] = frame['x1']
    frame.to_stata(target, write_index=False, version=118,
                   time_stamp=datetime.datetime(2000, 1, 1))
    metadata = json.loads(original.with_suffix('.json').read_text())
    hashes = {}
    for name in frame:
        values = frame[name].to_numpy(dtype='<f8', copy=True)
        values[np.isnan(values)] = float.fromhex('0x1p+1023')
        hashes[name] = hashlib.sha256(values.tobytes()).hexdigest()
    metadata.update(source_sha256=sha(original), sha256=sha(target),
                    column_sha256=hashes, transform='replace x2 with x1 exactly')
    metadata['recipe'] = dict(metadata['recipe'], duplicate_column='x2=x1')
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    return target

for spec in specs:
    data, warm = input_for(spec, SEED), input_for(spec, SEED+1, True)
    extra = {}
    if spec['name'] == 'rank_deficient_saved':
        data, warm = duplicate_column(data), duplicate_column(warm)
        extra['data_transform'] = 'replace x2 with x1 exactly in full and warm data'
    if spec['name'] == 'explicit_cg_saved':
        extra['solver'] = 'cg'
    record(dict(model='ols', section='width', **spec), data, warm, **extra)

assert len(requests) == 12
(ROOT/'control-requests.json').write_text(json.dumps(requests, indent=2)+'\n')
