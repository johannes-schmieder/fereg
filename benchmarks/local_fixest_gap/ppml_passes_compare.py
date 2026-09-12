"""Exact-output local PPML scratch/pass comparisons; no qualification claims."""
import argparse
import copy

from common import sha256, write_json
from measure import measure
from ppml_preparation_compare import PRIMARY, CONTROLS, compare, input_for, named_runtime
from run import BASE, MANIFEST

BASELINE = 'd32f05e45519aaee5c5d44bc87caddb4f68482f3'


def pair(spec, seed, rotation=0, audit=False, prefix=''):
    data, warm = input_for(spec, seed), input_for(spec, seed + 1, warm=True)
    cell = dict(model='ppml', section='width', **spec)
    cell['effects'] = ['g1'] if spec['design'] == 'one_fe' else ['g1', 'g2']
    command = 'feppml'
    if audit:
        command = '_fereg_estimate_ppml'
        weight = {'': '', '[fw=w]': 'fweight(w)', '[pw=w]': 'pweight(w)'}[spec.get('weight', '')]
        cell['weight'] = ''
        cell['fit_options'] = f"{spec.get('fit_options', '')} allocationaudit {weight}".strip()
    paths = {}
    for arm in (['candidate', 'baseline'] if rotation % 2 else ['baseline', 'candidate']):
        source = BASE / arm
        path = BASE / 'runs' / f'ppmlpasses-{prefix}{spec["name"]}-s{seed}-{arm}'
        paths[arm] = path
        manifest = copy.deepcopy(MANIFEST)
        manifest['measurement']['command_timeout_seconds'] = 120
        result = measure(BASE, cell, command, 'warm', data, warm, path, manifest,
            identity=dict(base_commit=BASELINE, variant=source.resolve().name, seed=seed,
                          allocation_audit=audit,
                          plugin_sha256=sha256(source / 'dist/fereg_macos.plugin')),
            source=source, trace=audit)
        print(path.name, result['status'], result['seconds'], flush=True)
        if result['status'] != 'pass':
            raise RuntimeError(path)
    compare(paths)
    if audit:
        runtime = {arm: named_runtime(path) for arm, path in paths.items()}
        for arm, values in runtime.items():
            for key in ('ppml_capacity_stable', 'linear_capacity_stable', 'audit_scratch_stable',
                        'linear_session_allocation_free', 'fresh_warm_equivalent'):
                assert values[key] == 1, (arm, key)
            assert values['audit_current_growth'] == values['audit_peak_growth'] == 0
            assert values['allocation_peak_delta'] <= values['modeled_selected_bytes']
            for key, value in values.items():
                if key.startswith('observed_peak_'):
                    assert value <= values[key.replace('observed_', 'modeled_')], (arm, key)
        assert runtime['candidate']['audit_events_delta'] <= runtime['baseline']['audit_events_delta']
        if spec['design'] == 'one_fe' and 'memory(low)' not in spec.get('fit_options', ''):
            for key in ('allocation_certificate_passed', 'iteration_allocation_free'):
                assert runtime['candidate'][key] == 1, key
            assert runtime['candidate']['audit_events_delta'] == 0
        write_json(paths['candidate'] / 'allocation-science.json', runtime)
    return paths


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--part', choices=['allocation', 'pilot', 'primary', 'controls', 'profile', 'boundary'], required=True)
    args = parser.parse_args()
    if args.part == 'allocation':
        for i, spec in enumerate(PRIMARY):
            pair(spec, 104729, rotation=i)
        for i, name in enumerate(('weak_d3', 'small_narrow', 'low_fw', 'saved_separation')):
            pair(next(s for s in CONTROLS if s['name'] == name), 31415927, rotation=i)
        for i, spec in enumerate((PRIMARY[2], CONTROLS[0], CONTROLS[7])):
            pair(spec, 67280421, rotation=i, audit=True, prefix='trace-')
    elif args.part == 'boundary':
        for i, rows in enumerate((65_535, 65_537)):
            pair(dict(name=f'norm_rows_{rows}', design='one_fe', n=rows, k=8, threads=4),
                 31415927, rotation=i)
    elif args.part == 'pilot':
        pair(PRIMARY[0], 104729, prefix='pilot-')
    elif args.part == 'primary':
        for i, spec in enumerate(PRIMARY):
            for j, seed in enumerate((104729, 8675309, 20260819)):
                pair(spec, seed, rotation=i+j)
    elif args.part == 'controls':
        for i, spec in enumerate(CONTROLS):
            pair(spec, 31415927, rotation=i)
    else:
        for i, spec in enumerate(PRIMARY):
            pair(spec, 67280421, rotation=i, audit=True, prefix='trace-')
