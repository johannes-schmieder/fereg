"""Local one-FE input-boundary probes; no cluster or remote execution."""
import argparse
import copy
import datetime
import hashlib
from pathlib import Path

import numpy as np
import pandas as pd

from common import sha256, write_json
from final_compare import baseline_compare, certify, invoke
from measure import measure
from run import BASE, MANIFEST, compare, data_for


def missing_data(n, k, seed):
    original, _ = data_for('one_fe', n, k, seed)
    directory = BASE / 'missing-data'
    directory.mkdir(exist_ok=True)
    path = directory / f'one-fe-sparse-missing-n{n}-k{k}-s{seed}.dta'
    if path.exists():
        return path
    frame = pd.read_stata(original, convert_categoricals=False)
    frame.loc[frame.index % 997 == 0, f'x{k}'] = np.nan
    frame.to_stata(path, write_index=False, version=118,
                   time_stamp=datetime.datetime(2000, 1, 1))
    # This probe compares Stata implementations only. Its import audit uses
    # Stata's double representation of system missing, not an IEEE NaN.
    hashes = {}
    with path.with_suffix('.bin').open('wb') as stream:
        for name in frame:
            values = frame[name].to_numpy(dtype='<f8', copy=True)
            values[np.isnan(values)] = float.fromhex('0x1p+1023')
            raw = values.tobytes()
            stream.write(raw)
            hashes[name] = hashlib.sha256(raw).hexdigest()
    write_json(path.with_suffix('.json'), {
        'schema': 'local-stata-missing-input-v1', 'n': n, 'k': k, 'seed': seed,
        'design': 'one_fe_sparse_missing', 'sha256': sha256(path),
        'columns': list(frame), 'column_sha256': hashes,
        'recipe': 'Frozen one_fe input; xK system-missing on zero-based rows divisible by 997.',
        'binary_missing_encoding': 'Stata system missing (2^1023); Stata-only probe',
    })
    return path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--part', choices=['one-fe', 'missing', 'profile'], required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    if args.part == 'one-fe':
        for n, k in [(983040, 1), (983040, 10), (7680, 50)]:
            paths = {label: invoke('final-extra-' + label, 'one_fe', n, k,
                     16, 104729, arm, source)
                     for label, arm, source in [('baseline', 'fereg', args.baseline),
                         ('candidate', 'fereg', args.source), ('fixest', 'fixest', args.baseline)]}
            baseline_compare(paths['candidate'], paths['baseline'])
            compare(paths['candidate'], paths['fixest'])
    elif args.part == 'profile':
        for design in ['one_fe', 'strong_d3', 'strong_d6']:
            for label, source in [('baseline', args.baseline), ('candidate', args.source)]:
                path = invoke('final-profile-' + label, design, 983040, 50,
                              16, 104729, 'fereg', source, trace=True)
                untraced = BASE / 'runs' / f'final-{label}-{design}-n983040-k50-t16-s104729-fereg'
                baseline_compare(path, untraced)
    else:
        for rep, seed in enumerate([104729, 8675309, 20260819]):
            data = missing_data(983040, 50, seed)
            warm, _ = data_for('one_fe', 7680, 50, seed + 1)
            cell = {'model': 'ols', 'design': 'one_fe_sparse_missing', 'n': 983040,
                    'k': 50, 'threads': 16, 'section': 'width', 'effects': ['g1']}
            arms = [('baseline', args.baseline), ('candidate', args.source)]
            arms = arms[rep % 2:] + arms[:rep % 2]
            paths = {}
            for label, source in arms:
                path = BASE / 'runs' / f'final-missing-{label}-n983040-k50-t16-s{seed}-fereg'
                paths[label] = path
                identity = {'candidate_base_commit': MANIFEST['candidate_commit'],
                            'variant': source.name, 'plugin_sha256': sha256(source / 'dist/fereg_macos.plugin'),
                            'trace': False, 'solver': 'auto', 'seed': seed}
                result = measure(BASE, cell, 'fereg', 'warm', data, warm, path,
                                 copy.deepcopy(MANIFEST), identity=identity, source=source)
                print(path.name, result['status'], result['seconds'], flush=True)
                write_json(path / 'native-science.json', certify(path))
            print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)


if __name__ == '__main__':
    main()
