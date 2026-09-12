"""Bounded local PPML selection/ownership comparisons; no qualification claims."""
import argparse
import copy
import datetime
import hashlib
import re
from pathlib import Path

import numpy as np
import pandas as pd

from common import read_json, sha256, write_json
from final_compare import matrix
from measure import measure
from preparation_compare import input_for as preparation_input
from run import BASE, MANIFEST

BASELINE = 'b19a1cfac038dcc6fbf015cfbfaf9f51022dafb4'
PRIMARY = [
    dict(name='one_fe_complete', design='one_fe', n=983040, k=50, threads=16),
    dict(name='fw_exposure_missing', design='one_fe', n=245760, k=50, threads=4,
         weight='[fw=w]', fit_options='exposure(exposure)', missing=True),
    dict(name='pw_cluster_offset', design='one_fe', n=245760, k=50, threads=16,
         weight='[pw=w]', vce='cluster c1 c2', fit_options='offset(off)', missing=True),
]
CONTROLS = [
    *[dict(name=name, design=name, n=7680, k=10, threads=4) for name in
      ('weak_d3', 'two_fe_dense', 'bottleneck_ring', 'cycle', 'disconnected_blocks', 'unbalanced')],
    dict(name='small_narrow', design='one_fe', n=7680, k=2, threads=1),
    dict(name='low_fw', design='one_fe', n=30720, k=10, threads=4,
         weight='[fw=w]', vce='cluster c1', fit_options='memory(low) exposure(exposure)', missing=True),
    dict(name='restricted', design='one_fe', n=61440, k=16, threads=4,
         restriction='if eligible', weight='[pw=w]', fit_options='offset(off)', missing=True),
    dict(name='saved_separation', design='one_fe', n=7680, k=3, threads=4,
         absorb='saved_g=g1', fit_options='offset(off) mu(saved_mu) dropreason(saved_drop)', zero_groups=True),
]


def input_for(spec, seed, warm=False):
    original = preparation_input(spec, seed, warm)
    path = BASE / 'ppml-data' / original.name
    if path.exists():
        return path
    frame = pd.read_stata(original, convert_categoricals=False)
    i = frame.obs_id.to_numpy(dtype=np.int64)
    eta = 0.15 * np.sin(frame.g1 / 7) + 0.06 * frame.x1 - 0.04 * frame.x2
    if 'g2' in frame:
        eta -= 0.12 * np.cos(frame.g2 / 5)
    frame['y'] = 1 + np.floor(100 * np.exp(eta) + i % 17) % 19
    frame['off'] = 0.1 * np.sin(i % 27)
    frame['exposure'] = np.exp(frame.off)
    if spec.get('zero_groups'):
        frame.loc[frame.g1 % 31 == 0, 'y'] = 0.0
    if spec.get('missing'):
        frame.loc[i % 977 == 0, ['off', 'exposure']] = np.nan
    path.parent.mkdir(parents=True, exist_ok=True)
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000, 1, 1))
    info = read_json(original.with_suffix('.json'))
    hashes = {}
    for name in frame:
        values = frame[name].to_numpy(dtype='<f8', copy=True)
        values[np.isnan(values)] = float.fromhex('0x1p+1023')
        hashes[name] = hashlib.sha256(values.tobytes()).hexdigest()
    info.update(schema='ppml-preparation-input-v1', model='ppml', source_sha256=sha256(original),
                sha256=sha256(path), columns=list(frame), column_sha256=hashes)
    write_json(path.with_suffix('.json'), info)
    return path


def named_runtime(path):
    ado = (BASE / 'baseline/stata/_fereg_estimate_ppml.ado').read_text()
    names = re.search(r"matrix rownames `runtime_shadow' = (.*?)matrix colnames `runtime_shadow'", ado, re.S)[1]
    names = names.replace('///', '').split()
    values = matrix(path, 'feppml_runtime')[:, 0]
    assert len(names) == len(values)
    return {name: None if str(value).strip() == '.' else float(value)
            for name, value in zip(names, values)}


def compare(paths):
    a, b = paths['candidate'], paths['baseline']
    ra, rb = read_json(a / 'result.json'), read_json(b / 'result.json')
    assert ra['status'] == rb['status'] == 'pass'
    assert ra['sample_sha256'] == rb['sample_sha256']
    runtime = {arm: named_runtime(path) for arm, path in paths.items()}
    for name in ('b', 'V', 'feppml_status', 'feppml_fit', 'feppml_dof', 'feppml_fe',
                 'feppml_cluster', 'feppml_subset', 'feppml_separation', 'feppml_inference',
                 'feppml_drop_counts', 'feppml_separation_sequence', 'feppml_certificate', 'feppml_receipt'):
        assert (a / f'{name}.csv').read_bytes() == (b / f'{name}.csv').read_bytes(), name
    solver = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 16, 17, 18, 19, 20, 21]
    assert np.array_equal(matrix(a, 'feppml_solver')[:, solver], matrix(b, 'feppml_solver')[:, solver])
    for key in ('outer_iterations', 'inner_iterations', 'step_halvings', 'memory_requested',
                'memory_selected', 'linear_route', 'fallback_mask', 'fallback_count',
                'threads_requested', 'threads_effective', 'modeled_selected_bytes'):
        assert runtime['candidate'][key] == runtime['baseline'][key], key
    for path in paths.values():
        assert matrix(path, 'feppml_certificate')[-1, 0] == 1
        assert read_json(path / 'result.json')['metadata']['converged'] == 1
    if (a / 'saved_outputs.csv').exists():
        assert (a / 'saved_outputs.csv').read_bytes() == (b / 'saved_outputs.csv').read_bytes()
    write_json(a / 'exact-science.json', dict(sample_b_V_status_dof_weight_cluster_separation_exact=True,
               certificates_pass=True, solver_decisions_exact=True, saved_outputs_exact=True))
    write_json(a / 'paired-runtime.json', runtime)


def pair(spec, seed, rotation=0, trace=False, prefix=''):
    data, warm = input_for(spec, seed), input_for(spec, seed + 1, warm=True)
    cell = dict(model='ppml', section='width', **spec)
    cell['effects'] = ['g1'] if spec['design'] == 'one_fe' else ['g1', 'g2']
    command = 'feppml'
    if spec.get('private_allocation_audit'):
        command = '_fereg_estimate_ppml'
        weight_option = {'': '', '[fw=w]': 'fweight(w)', '[pw=w]': 'pweight(w)'}[spec.get('weight', '')]
        cell['weight'] = ''
        cell['fit_options'] = f"{spec.get('fit_options', '')} allocationaudit {weight_option}".strip()
    paths = {}
    for arm in (['candidate', 'baseline'] if rotation % 2 else ['baseline', 'candidate']):
        source = BASE / arm
        path = BASE / 'runs' / f'ppmlprep-{prefix}{spec["name"]}-s{seed}-{arm}'
        paths[arm] = path
        manifest = copy.deepcopy(MANIFEST)
        manifest['measurement']['command_timeout_seconds'] = 120
        result = measure(BASE, cell, command, 'warm', data, warm, path, manifest,
                         identity=dict(base_commit=BASELINE, variant=arm, seed=seed, trace=trace,
                                       plugin_sha256=sha256(source / 'dist/fereg_macos.plugin')),
                         source=source, trace=trace)
        print(path.name, result['status'], result['seconds'], flush=True)
        if result['status'] != 'pass':
            raise RuntimeError(path)
    compare(paths)
    return paths


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--part', choices=['pilot', 'primary', 'controls', 'profile'], required=True)
    args = parser.parse_args()
    if args.part == 'pilot':
        pair(PRIMARY[0], 104729, prefix='pilot-')
    elif args.part == 'primary':
        for j, spec in enumerate(PRIMARY):
            for r, seed in enumerate([104729, 8675309, 20260819]):
                pair(spec, seed, rotation=j + r)
    elif args.part == 'controls':
        for r, spec in enumerate(CONTROLS):
            pair(spec, 31415927, rotation=r)
    else:
        for r, spec in enumerate(PRIMARY):
            audited = dict(spec, private_allocation_audit=True)
            pair(audited, 67280421, rotation=r, trace=True, prefix='trace-')
