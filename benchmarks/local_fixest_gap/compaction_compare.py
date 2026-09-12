"""Sequential local incomplete-sample and complete one-FE comparisons.

FEREG_LOCAL_BENCH_ROOT holds immutable baseline and candidate trees. Missing
binaries encode Stata system missing and must never be sent to fixest.
"""
import argparse
import copy
import datetime
import hashlib

import numpy as np
import pandas as pd

from common import read_json, sha256, write_json
from final_compare import baseline_compare, certify, invoke
from input_compare import missing_data
from measure import measure
from run import BASE, MANIFEST, compare, data_for


def filtered_data(seed, paired=True):
    original, _ = data_for('one_fe', 245760, 50, seed)
    directory = BASE / 'filtered-data'
    directory.mkdir(exist_ok=True)
    label = 'paired' if paired else 'heavy'
    path = directory / f'one-fe-{label}-missing-n245760-k50-s{seed}.dta'
    if path.exists():
        return path
    frame = pd.read_stata(original, convert_categoricals=False)
    retained_slots = 2 if paired else 1
    frame.loc[frame.index % 20 >= retained_slots, 'x50'] = np.nan
    frame.to_stata(path, write_index=False, version=118,
                   time_stamp=datetime.datetime(2000, 1, 1))
    hashes = {}
    with path.with_suffix('.bin').open('wb') as stream:
        for name in frame:
            values = frame[name].to_numpy(dtype='<f8', copy=True)
            values[np.isnan(values)] = float.fromhex('0x1p+1023')
            raw = values.tobytes()
            stream.write(raw)
            hashes[name] = hashlib.sha256(raw).hexdigest()
    write_json(path.with_suffix('.json'), {
        'schema': 'local-stata-missing-input-v1', 'n': len(frame), 'k': 50,
        'seed': seed, 'design': f'one_fe_{label}_missing', 'sha256': sha256(path),
        'columns': list(frame), 'column_sha256': hashes,
        'recipe': f'Frozen one_fe; x50 missing unless zero-based row modulo 20 is below {retained_slots}.',
        'binary_missing_encoding': 'Stata system missing (2^1023); Stata-only probe',
    })
    return path


def missing_pair(part, threads, seed, rep, run_part=None):
    n = 983040 if part == 'missing' else 245760
    design = 'one_fe_sparse_missing' if part == 'missing' else 'one_fe_paired_missing'
    data = missing_data(n, 50, seed) if part == 'missing' else filtered_data(seed)
    warm, _ = data_for('one_fe', 7680, 50, seed + 1)
    cell = {'model': 'ols', 'design': design, 'n': n, 'k': 50,
            'threads': threads, 'section': 'width', 'effects': ['g1']}
    if run_part is None:
        run_part = 'filtered-paired' if part == 'filtered' else part
    paths = {}
    arms = ['baseline', 'candidate']
    arms = arms[rep % 2:] + arms[:rep % 2]
    for name in arms:
        source = BASE / name
        path = BASE / 'runs' / f'final-{run_part}-{name}-n{n}-k50-t{threads}-s{seed}-fereg'
        paths[name] = path
        identity = {'candidate_base_commit': MANIFEST['candidate_commit'],
                    'variant': name, 'plugin_sha256': sha256(source / 'dist/fereg_macos.plugin'),
                    'trace': False, 'solver': 'auto', 'seed': seed}
        result = measure(BASE, cell, 'fereg', 'warm', data, warm, path,
                         copy.deepcopy(MANIFEST), identity=identity, source=source)
        print(path.name, result['status'], result['seconds'], flush=True)
        write_json(path / 'native-science.json', certify(path))
    print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)


def all_singletons():
    seed = 59047
    data = filtered_data(seed, paired=False)
    frame = pd.read_stata(data, columns=['g1', 'x50'], convert_categoricals=False)
    retained = frame.dropna()
    counts = retained.groupby('g1').size()
    assert (counts == 1).all() and len(retained) == 12288
    warm, _ = data_for('one_fe', 7680, 50, seed + 1)
    cell = {'model': 'ols', 'design': 'one_fe_heavy_missing', 'n': 245760,
            'k': 50, 'threads': 4, 'section': 'width', 'effects': ['g1']}
    checks = {}
    for name in ['baseline', 'candidate']:
        source = BASE / name
        # Preserve the original unsuccessful benchmark attempt as evidence.
        label = 'final-filtered-baseline' if name == 'baseline' else 'negative-singleton-candidate'
        path = BASE / 'runs' / f'{label}-n245760-k50-t4-s{seed}-fereg'
        identity = {'candidate_base_commit': MANIFEST['candidate_commit'],
                    'variant': name, 'plugin_sha256': sha256(source / 'dist/fereg_macos.plugin'),
                    'trace': False, 'solver': 'auto', 'seed': seed}
        result = read_json(path / 'result.json') if path.exists() else measure(
            BASE, cell, 'fereg', 'warm', data, warm, path,
            copy.deepcopy(MANIFEST), identity=identity, source=source)
        assert result['input_sha256'] == sha256(data)
        assert result['measurement_fingerprint']['plugin_sha256'] == identity['plugin_sha256']
        assert result['sentinel'] and result['metadata']['rc'] == 2001
        checks[path.name] = {'stata_rc': 2001, 'expected': 'insufficient observations after singleton deletion'}
    write_json(BASE / 'singleton-check.json', {
        'input_sha256': sha256(data), 'original_rows': len(frame),
        'available_rows': len(retained), 'available_FE_groups': len(counts),
        'all_available_rows_are_singletons': True, 'checks': checks,
        'note': 'Original worker_failure receipts remain unchanged; these are negative controls, not runtime successes.',
    })
    print('ALL-SINGLETON CONTROL: both return 2001', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--part', required=True,
                        choices=['missing', 'filtered', 'primary', 'all-singletons', 'recheck'])
    args = parser.parse_args()
    if args.part == 'all-singletons':
        all_singletons()
    elif args.part == 'recheck':
        missing_pair('missing', 4, 104729, 1, run_part='recheck-missing')
    elif args.part in ['missing', 'filtered']:
        seeds = [104729, 8675309, 20260819] if args.part == 'missing' else [59047]
        for threads in [4, 16]:
            for rep, seed in enumerate(seeds):
                missing_pair(args.part, threads, seed, rep)
    else:
        for rep, seed in enumerate([104729, 8675309, 20260819]):
            arms = [('baseline', 'fereg'), ('candidate', 'fereg'), ('fixest', 'fixest')]
            arms = arms[rep % 3:] + arms[:rep % 3]
            paths = {name: invoke('final-' + name, 'one_fe', 983040, 50,
                     16, seed, arm, BASE / ('baseline' if name == 'fixest' else name))
                     for name, arm in arms}
            print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)
            print('FIXEST', compare(paths['candidate'], paths['fixest']), flush=True)


if __name__ == '__main__':
    main()
