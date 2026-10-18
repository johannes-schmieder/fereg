#!/usr/bin/env python3
"""Prepare a sparse, fixed routing panel through maintained native generators.

No-FE, one-FE and slope cases must be supplied explicitly: replay_large cannot
create those shapes. Missing witnesses stop preparation; they are never silently
removed from coverage. Generated cases are inputs, not scientific evidence.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct

import numpy as np
import subprocess

from cmgschur_cases import (HIGH_DEGREE_SIZES, additional_case, graph_summary, high_degree_case,
                            synthetic, write_case)
from native_io import ROOT, case_header, digest

REQUIRED_IMPORTS = ('no-fe', 'one-fe', 'slopes')
NATIVE_CASES = {
    'dense': {'rows': 100000, 'levels': '500,100', 'regressors': 5, 'weights': 'unit'},
    'small-retained': {'rows': 60000, 'levels': '3000,20', 'regressors': 5, 'weights': 'unit'},
    'small-third': {'rows': 100000, 'levels': '10000,1000,20', 'regressors': 5, 'weights': 'unit'},
    'four-fe': {'rows': 100000, 'levels': '5000,500,100,20', 'regressors': 5, 'weights': 'unit'},
    'eight-fe': {'rows': 100000, 'levels': '5000,500,100,50,40,30,20,10', 'regressors': 5, 'weights': 'unit'},
    'weighted': {'rows': 100000, 'levels': '10000,1000', 'regressors': 5, 'weights': 'pw'},
}
SYNTHETIC_CASES = {'weak': ('weak', 5000, 500, 20),
                   'repeated-cells': ('long', 5000, 500, 20),
                   'wide-rhs': ('wide', 5000, 500, 20)}
EXTENDED_CASES = ('sixteen-fe', 'disconnected', 'near-confounded-third', 'weight-dynamic', 'inactive-columns')
SAMPLED_CASES = {'high-degree-mixed': 'd30-mixed', 'high-degree-weak': 'd30-weak',
                 'high-degree-stress': 'd60-stress'}
SMALL_HIGH_DEGREE = {'d30-mixed': (42, 40, 31), 'd30-weak': (320, 300, 31), 'd60-stress': (82, 80, 61)}


def scratch(path):
    path = path.resolve()
    if not path.is_relative_to((ROOT / 'target/tmp').resolve()) or path == (ROOT / 'target/tmp').resolve():
        raise ValueError('output must be under one task directory in target/tmp')
    return path


def extended_columns(family, seed, workers=5000, firms=500, periods=20):
    if family == 'inactive-columns':
        return additional_case('orthogonal', workers, firms, periods, seed), None, 2, False
    if family == 'disconnected':
        halves = [synthetic('long', workers // 2, firms // 2, periods, seed + offset) for offset in [0, 1]]
        halves[1][-2] += workers // 2
        halves[1][-1] += firms // 2
        return [np.concatenate(parts) for parts in zip(*halves)], None, 2, True
    columns = synthetic('weak' if family == 'weight-dynamic' else 'long', workers, firms, periods, seed)
    rng = np.random.default_rng(seed + 17)
    if family == 'weight-dynamic':
        return columns, np.exp(rng.uniform(-8., 8., len(columns[0]))), 2, True
    if family == 'near-confounded-third':
        year = columns[-1].astype(np.int64) % 20
        movers = rng.random(len(year)) < .005
        year[movers] = rng.integers(0, 20, movers.sum())
        return [*columns, year.astype(float)], None, 3, True
    raise ValueError('unknown extended family')


def copy_import(source, target, low=False):
    with source.open('rb') as stream, target.open('xb') as out:
        shape, args = case_header(stream)
        if low:
            if sum(a.startswith(b'memory=') for a in args) != 1:
                raise ValueError('Low-memory fixture requires one canonical memory option')
            args = [b'memory=low' if a.startswith(b'memory=') else a for a in args]
        out.write(('FEREG_REPLAY_V1\n' + ' '.join(map(str, shape)) + '\n').encode())
        for arg in args:
            out.write(str(len(arg)).encode() + b'\n' + arg + b'\n')
        shutil.copyfileobj(stream, out, 1 << 20)


def sources(items):
    out = {}
    for item in items:
        family, path = item.split('=', 1)
        if family not in REQUIRED_IMPORTS or family in out:
            raise ValueError('imports must name each of no-fe, one-fe and slopes exactly once')
        out[family] = Path(path).resolve(strict=True)
    if set(out) != set(REQUIRED_IMPORTS) or len(set(out.values())) != len(out):
        raise ValueError('require distinct explicit no-fe, one-fe and slopes inputs')
    return out


def inspect_case(path, family=None):
    with path.open('rb') as stream:
        shape, args = case_header(stream)
        body_hash = hashlib.sha256()
        for block in iter(lambda: stream.read(1 << 20), b''):
            body_hash.update(block)
    if args[0] != b'estimate-ols':
        raise ValueError('routing inputs must use OLS')
    fields = dict(a.decode().split('=', 1) for a in args[1:])
    saved = []
    encoded = fields.get('output_saved_channels', '.')
    if encoded != '.':
        for item in encoded.split('|'):
            length, value = item.split(':', 1)
            if int(length) != len(value) or not value.isdigit():
                raise ValueError('invalid saved-output registry')
            position = int(value)
            if position:
                saved.append(position - 1)
        if len(set(saved)) != len(saved) or any(position >= shape[1] for position in saved):
            raise ValueError('invalid saved-output columns')
    count = int(fields['absorb_term_count'])
    if family in {'no-fe', 'one-fe'} and count != int(family == 'one-fe'):
        raise ValueError(f'{family}: imported absorbed-term count does not match family')
    if family == 'slopes' and count == 0:
        raise ValueError('imported slope fixture has no slope terms')
    categorical = count == 0
    if count > 0:
        # Inspect the canonical absorbed-term table, never infer slope coverage
        # from a filename. Native benchmark inputs are numeric fixed-width hex.
        with path.open('rb') as stream:
            case_header(stream)
            for _ in range(shape[1]):
                if stream.readline() != b'0\n':
                    raise ValueError('imported routing fixtures must use numeric replay columns')
                stream.seek(shape[0] * 17, 1)
            terms = None
            for _ in range(shape[3]):
                length = int(stream.readline())
                if not 0 < length <= 1000:
                    raise ValueError('invalid matrix name frame')
                name = stream.read(length).decode()
                if stream.read(1) != b'\n':
                    raise ValueError('invalid matrix name delimiter')
                rows, cols = map(int, stream.readline().split())
                if name == fields['absorb_term_matrix']:
                    if cols != 4 or rows != max(1, count):
                        raise ValueError('invalid absorbed-term table shape')
                    terms = [[struct.unpack('>d', bytes.fromhex(stream.readline().decode().strip()))[0]
                              for _ in range(cols)] for _ in range(rows)]
                else:
                    stream.seek(rows * cols * 17, 1)
            if terms is None or (family == 'slopes' and not any(t[3] > 0 for t in terms)):
                raise ValueError('imported slope fixture has no slope terms')
            categorical = all(term[2] == 1 and term[3] == 0 for term in terms)
            if family == 'one-fe' and not categorical:
                raise ValueError('one-fe fixture must have one categorical intercept and no slopes')
    return {'rows': shape[0], 'columns': shape[1], 'absorbed_terms': count,
            'categorical_intercepts': categorical,
            'body_sha256': body_hash.hexdigest(), 'saved': saved,
            'outputs': {name: int(fields['output_' + name]) - 1 for name in
                        ['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed']}}


def paired_orthogonal_oracle(columns):
    """FWL truth when every y/X pair cancels inside the same worker/firm cell."""
    y, *xs, worker, firm = columns
    values = np.asarray([y, *xs])
    if (len(y) % 2 or not np.all(np.isfinite(columns))
            or not np.array_equal(values[:, ::2], -values[:, 1::2])
            or not np.array_equal(worker[::2], worker[1::2])
            or not np.array_equal(firm[::2], firm[1::2])):
        raise ValueError('paired within-cell cancellation is not exact')
    x = np.asarray(xs).T
    if np.linalg.matrix_rank(x) != len(xs):
        raise ValueError('orthogonal controls are not full rank')
    _, wi = np.unique(worker, return_inverse=True)
    _, fi = np.unique(firm, return_inverse=True)
    width = int(wi.max()) + 1
    parent = list(range(width + int(fi.max()) + 1))
    def root(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i
    for a, b in zip(wi[::2], fi[::2]):
        a, b = root(int(a)), root(width + int(b))
        parent[b] = a
    components = len({root(i) for i in range(len(parent))})
    rank = len(parent) - components + len(xs)
    if rank >= len(y):
        raise ValueError('orthogonal fixture has no residual degrees of freedom')
    beta = np.linalg.lstsq(x, y, rcond=1e-13)[0]
    fitted = x @ beta
    residual = y - fitted
    inverse = np.linalg.solve(x.T @ x, x.T)
    covariance = (inverse * residual) @ (inverse * residual).T * len(y) / (len(y) - rank)
    return {'method': 'exact paired FE orthogonality, independent NumPy OLS/HC1 and bipartite graph rank',
            'rank': rank, 'components': components, 'regressors': len(xs), 'beta': beta.tolist(),
            'covariance': covariance.tolist(), 'fitted': fitted.tolist(), 'residual': residual.tolist()}


def inactive_case_oracle(path):
    """Inspect the frozen canonical fixture itself; never infer truth from its name."""
    with path.open('rb') as stream:
        shape, args = case_header(stream)
        fields = dict(a.decode().split('=', 1) for a in args[1:])
        if (shape[1] != 14 or not 2 <= shape[0] <= 1_000_000 or shape[4] != 0
                or any(fields.get(k) != v for k, v in {'include_intercept': '0', 'vce': 'robust',
                    'absorb_term_count': '2', 'term_count': '4', 'binding_count': '8',
                    'cluster_term_count': '0', 'dof': 'pairwise', 'account_cluster_nesting': '0'}.items())):
            raise ValueError('not the supported canonical unweighted four-control orthogonal fixture')
        data = []
        for i in range(shape[1]):
            if stream.readline() != b'0\n':
                raise ValueError('orthogonal witness requires numeric inputs')
            if i < 8:
                data.append(np.frombuffer(bytes.fromhex(stream.read(shape[0] * 17).decode()), dtype='>f8').astype(float))
            else:
                stream.seek(shape[0] * 17, 1)
        tables = {}
        for _ in range(shape[3]):
            length = int(stream.readline())
            name = stream.read(length).decode()
            if stream.read(1) != b'\n':
                raise ValueError('invalid matrix frame')
            rows, cols = map(int, stream.readline().split())
            tables[name] = np.frombuffer(bytes.fromhex(stream.read(rows * cols * 17).decode()), dtype='>f8').reshape(rows, cols)
    expected = {
        'S': [[j, 0] for j in range(1, 7)],
        'T': [[j, j, j, 1, 1, 0, 0, 0] for j in range(1, 5)],
        'C': [[j, 1, j, 1, 1, 0, 0, 2, 1, j, j, 0, 0] for j in range(1, 5)],
        'A': [[1, 1, 1, 0], [2, 1, 1, 0]], 'AC': [[1, 1, 1, 5], [2, 1, 1, 6]],
        'BIND': [[1, 1, 0], [2, 2, 0], *[[j + 2, 3, j] for j in range(1, 7)]]}
    names = {'S': 'source_matrix', 'T': 'term_matrix', 'C': 'component_matrix',
             'A': 'absorb_term_matrix', 'AC': 'absorb_component_matrix', 'BIND': 'binding_matrix'}
    if (not np.all(data[0] == 1.) or any(not np.array_equal(tables.get(fields.get(names[name])), value)
                                      for name, value in expected.items())):
        raise ValueError('orthogonal input/model binding differs from the independent witness')
    return paired_orthogonal_oracle(data[1:])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seed', type=int, required=True, help='unsigned 64-bit generation seed')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--generator', type=Path, default=ROOT / 'target/ci/examples/replay_large')
    parser.add_argument('--source', action='append', default=[], help='family=path for each required import')
    parser.add_argument('--threads', nargs='+', type=int, default=[1, 4])
    parser.add_argument('--low-saved', type=Path, required=True, help='independent native replay input with saved effects')
    parser.add_argument('--small', action='store_true', help='small generated fixtures for tooling smoke checks, not performance qualification')
    args = parser.parse_args()
    if not 0 <= args.seed <= (1 << 64) - 1:
        parser.error('--seed must be an unsigned 64-bit integer')
    try:
        imported = sources(args.source)
    except ValueError as error:
        parser.error(str(error))
    if not args.threads or len(set(args.threads)) != len(args.threads) or any(not 1 <= t <= 1024 for t in args.threads):
        parser.error('require distinct workers in 1..1024')
    imported['low-saved'] = args.low_saved.resolve(strict=True)
    generator = args.generator.resolve(strict=True)
    generator_hash = digest(generator)
    imported_hashes = {name: digest(path) for name, path in imported.items()}
    output = scratch(args.output)
    output.mkdir(parents=True, exist_ok=False)
    seed = args.seed
    cells = []
    for family in [*imported, *NATIVE_CASES, *SYNTHETIC_CASES, *SAMPLED_CASES, *EXTENDED_CASES]:
        path = output / f'{family}.case'
        metadata = {'schema': 1, 'family': family, 'coverage': [family], 'small': args.small,
                    'evidence': 'unqualified input; separate scientific witnesses required'}
        if family in imported:
            copy_import(imported[family], path, low=family == 'low-saved')
            if digest(imported[family]) != imported_hashes[family] or (family != 'low-saved' and digest(path) != imported_hashes[family]):
                raise ValueError('import changed while copying')
            metadata['source_sha256'] = imported_hashes[family]
            reference = imported[family].with_suffix('.json')
            if reference.is_file():
                copied = path.with_suffix('.reference.json')
                shutil.copyfile(reference, copied)
                metadata['replay_reference'] = {'file': copied.name, 'sha256': digest(copied)}
        elif family in NATIVE_CASES or family == 'sixteen-fe':
            spec = (NATIVE_CASES[family] if family in NATIVE_CASES else
                    {'rows': 100000, 'levels': '5000,500,100,50,40,30,20,10,9,8,7,6,5,4,3,2',
                     'regressors': 5, 'weights': 'unit'})
            if args.small:
                spec = {**spec, 'rows': 1200, 'levels': ','.join(str(min(int(n), 20)) for n in spec['levels'].split(','))}
            command = [str(generator), '--output', str(path), '--seed', str(seed), '--solver', 'auto',
                       '--vce', 'robust', '--graph', 'random-mobility', '--tolerance', '1e-8']
            for key, value in spec.items():
                command += ['--' + key, str(value)]
            subprocess.run(command, check=True, timeout=120, stdout=subprocess.DEVNULL)
            metadata.update(generator_sha256=generator_hash, seed=seed, generator_options=spec)
        elif family in EXTENDED_CASES:
            columns, weights, count, intercept = extended_columns(family, seed, **({'workers': 60, 'firms': 20} if args.small else {}))
            write_case(generator, path, columns, include_intercept=intercept, effect_count=count, weights=weights)
            metadata.update(generator_sha256=generator_hash, seed=seed, include_intercept=intercept,
                            effect_count=count, weight_kind='unit' if weights is None else 'pw')
        else:
            if family in SAMPLED_CASES:
                kind = SAMPLED_CASES[family]
                sizes = SMALL_HIGH_DEGREE[kind] if args.small else HIGH_DEGREE_SIZES[kind]
                columns = high_degree_case(kind, *sizes, seed=seed)
            else:
                kind, workers, firms, periods = SYNTHETIC_CASES[family]
                if args.small:
                    workers, firms = 60, 20
                function = additional_case if kind == 'wide' else synthetic
                columns = function(kind, workers, firms, periods, seed=seed)
            write_case(generator, path, columns)
            metadata.update(graph_summary(columns))
            metadata.update(generator_sha256=generator_hash, seed=seed)
        metadata.update(inspect_case(path, family))
        if family == 'low-saved':
            with path.open('rb') as stream:
                _, options = case_header(stream)
            fields = dict(a.decode().split('=', 1) for a in options[1:])
            encoded = fields.get('output_saved_channels', '.')
            if encoded == '.' or not any(part.split(':', 1)[-1] != '0' for part in encoded.split('|')):
                raise ValueError('--low-saved input does not request saved effects')
        metadata['case_sha256'] = digest(path)
        path.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
        for threads in args.threads:
            cells.append({'id': f'{family}-t{threads}', 'case_sha256': metadata['case_sha256'],
                          'body_sha256': metadata['body_sha256'],
                          'family': family, 'coverage': [family], 'threads': threads,
                          'absorbed_terms': metadata['absorbed_terms'],
                          'categorical_intercepts': metadata['categorical_intercepts'], 'case_file': path.name})
    if digest(generator) != generator_hash:
        raise ValueError('generator changed while preparing panel')
    (output / 'panel.json').write_text(json.dumps({'schema': 1, 'seed': seed, 'small': args.small,
        'cells': cells}, indent=2) + '\n')
    print(f'{len(cells)} declared cells prepared; no scientific qualification or timings performed')


if __name__ == '__main__':
    main()
