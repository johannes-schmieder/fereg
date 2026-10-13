#!/usr/bin/env python3
"""Build reproducible two-FE prototype cases, or convert a local numeric CSV.

Requires NumPy and the existing fereg-oracle replay_large executable. Outputs
are private scratch inputs under target/tmp/cmgschur; only aggregate metadata
belongs in retained evidence. CSV rows/outcomes are preserved without selection
or transformation. A small synthetic case includes an independent dense oracle.
"""
from __future__ import annotations

import argparse
from array import array
import csv
import json
from pathlib import Path
import subprocess

import numpy as np

from native_io import ROOT, case_header, digest


def scratch(path):
    path = path.resolve()
    if not path.is_relative_to((ROOT / 'target/tmp/cmgschur').resolve()):
        raise ValueError('output must be under target/tmp/cmgschur')
    return path


def synthetic(kind, workers, firms, periods, seed=20261013):
    """Connected panel; distinct employers differ from physical observations."""
    if kind not in {'short', 'long', 'weak'}:
        raise ValueError('unknown synthetic topology')
    if workers < firms + 10 or firms < 20 or periods < 2:
        raise ValueError('require workers >= firms+10, firms>=20, periods>=2')
    rng = np.random.default_rng(seed)
    worker = np.repeat(np.arange(workers), periods)
    employer = np.empty((workers, periods), dtype=np.int64)
    degrees = rng.choice(np.arange(1, 11), workers,
                         p=[.35, .20, .15, .10, .07, .05, .03, .025, .015, .01])
    if kind == 'short':
        degrees = 1 + (rng.random(workers) < .35)
    else:
        degrees[-10:] = np.arange(1, 11)
    groups = 10 if kind == 'weak' else 1
    if firms % groups:
        raise ValueError('weak graph requires a firm count divisible by ten')
    width = firms // groups
    for i in range(workers):
        group = i % groups
        if i < firms:  # Within-group rings ensure every firm is connected.
            first = i // groups if groups > 1 else i
            visits = np.array([group * width + first,
                               group * width + (first + 1) % width])
        elif groups > 1 and i < firms + groups - 1:
            group = i - firms
            visits = np.array([group * width, (group + 1) * width])
        else:
            degree = min(int(degrees[i]), periods, width)
            visits = group * width + rng.choice(width, degree, replace=False)
        employer[i] = np.resize(visits, periods)
    employer = employer.ravel()
    x = rng.standard_normal((2, worker.size))
    x[0] += .2 * np.sin(worker)
    y = (.7 * x[0] - .25 * x[1] + np.sin(worker * .071)
         + .6 * np.cos(employer * .13) + .3 * rng.standard_normal(worker.size))
    return [y, *x, worker.astype(float), employer.astype(float)]


def csv_columns(path, outcome, worker, firm, regressors):
    names = [outcome, *regressors, worker, firm]
    if len(set(names)) != len(names):
        raise ValueError('CSV columns must have distinct roles')
    values = [array('d') for _ in names]
    with path.open(newline='') as stream:
        reader = csv.DictReader(stream)
        if not reader.fieldnames or any(name not in reader.fieldnames for name in names):
            raise ValueError('requested column missing from CSV header')
        for row in reader:
            for column, name in zip(values, names):
                value = float(row[name])
                if not np.isfinite(value) or abs(value) >= 8e307:
                    raise ValueError('CSV must contain finite numeric values; no implicit deletion')
                column.append(value)
    columns = [np.frombuffer(value, dtype=np.float64) for value in values]
    if not columns[0].size:
        raise ValueError('empty CSV')
    for ids in columns[-2:]:
        if np.any(ids != np.floor(ids)) or np.any(np.abs(ids) > 2**53):
            raise ValueError('worker and firm IDs must be exactly represented integers')
    return columns


def graph_summary(columns):
    pairs = np.unique(np.column_stack(columns[-2:]), axis=0)
    _, degree = np.unique(pairs[:, 0], return_counts=True)
    values, counts = np.unique(degree, return_counts=True)
    return {'rows': len(columns[0]), 'workers': len(degree),
            'firms': len(np.unique(pairs[:, 1])), 'distinct_links': len(pairs),
            'degree_histogram': {str(d): int(n) for d, n in zip(values, counts)}}


def dense_oracle(columns):
    """Dense dummy OLS and HC1 covariance; no fereg solver is used."""
    y, *rest = columns
    xs, worker, firm = rest[:-2], rest[-2], rest[-1]
    worker_levels, wi = np.unique(worker, return_inverse=True)
    firm_levels, fi = np.unique(firm, return_inverse=True)
    design = np.column_stack([np.ones(len(y)), *xs,
                              np.eye(len(worker_levels))[wi, 1:],
                              np.eye(len(firm_levels))[fi, 1:]])
    inverse = np.linalg.pinv(design, rcond=1e-13)
    beta = inverse @ y
    fitted = design @ beta
    residual = y - fitted
    rank = int(np.linalg.matrix_rank(design))
    if rank != design.shape[1] or rank >= len(y):
        raise ValueError('dense fixture must be connected and have full design rank')
    covariance = (inverse * residual) @ (inverse * residual).T * len(y) / (len(y) - rank)
    return {'method': 'NumPy dense dummy SVD and HC1, one firm and worker base',
            'rank': rank, 'regressors': len(xs), 'beta': beta[1:1+len(xs)].tolist(),
            'covariance': covariance[1:1+len(xs), 1:1+len(xs)].tolist(),
            'fitted': fitted.tolist(), 'residual': residual.tolist()}


def write_case(generator, path, columns, tolerance=1e-8):
    """Use the canonical Rust serializer for metadata, replace only numeric data."""
    template = path.with_suffix('.template.case')
    regressors = len(columns) - 3
    subprocess.run([str(generator), '--output', str(template), '--rows', '80',
                    '--levels', '20,10', '--regressors', str(regressors),
                    '--weights', 'unit', '--vce', 'robust', '--graph', 'random-mobility',
                    '--solver', 'auto', '--seed', '20261013', '--tolerance', str(tolerance)],
                   check=True, stdout=subprocess.DEVNULL, timeout=60)
    try:
        with template.open('rb') as stream:
            shape, arguments = case_header(stream)
            for _ in range(shape[1]):
                if stream.readline() != b'0\n':
                    raise ValueError('numeric template required')
                for _ in range(shape[0]):
                    stream.readline()
            metadata = stream.read()
        fields = dict(a.decode().split('=', 1) for a in arguments[1:])
        n = len(columns[0])
        data = [np.ones(n), *columns]
        if len(data) != int(fields['output_sample']) - 1:
            raise ValueError('canonical template input layout changed')
        for name in ['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed']:
            initial = 1.0 if name == 'sample' else 0.0 if name == 'drop_reason' else float.fromhex('0x1p+1023')
            if int(fields['output_' + name]) != len(data) + 1:
                raise ValueError('canonical template output layout changed')
            data.append(np.full(n, initial))
        if len(data) != shape[1]:
            raise ValueError('canonical template width changed')
        with path.open('xb') as stream:
            stream.write(('FEREG_REPLAY_V1\n' + ' '.join(map(str, [n, *shape[1:]])) + '\n').encode())
            for value in arguments:
                stream.write(str(len(value)).encode() + b'\n' + value + b'\n')
            for column in data:
                stream.write(b'0\n')
                for first in range(0, n, 32768):
                    bits = np.asarray(column[first:first+32768], dtype=np.float64).view(np.uint64)
                    stream.write(''.join(f'{int(value):016x}\n' for value in bits).encode())
            stream.write(metadata)
        return {name: int(fields['output_' + name]) - 1
                for name in ['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed']}
    finally:
        template.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--generator', type=Path, default=ROOT / 'target/ci/examples/replay_large')
    parser.add_argument('--csv', type=Path)
    parser.add_argument('--outcome')
    parser.add_argument('--worker')
    parser.add_argument('--firm')
    parser.add_argument('--regressors', nargs='*', default=[])
    parser.add_argument('--tolerance', type=float, default=1e-8)
    args = parser.parse_args()
    if not 0 < args.tolerance < 1 or (args.csv and not all([args.outcome, args.worker, args.firm])):
        parser.error('require valid tolerance and all three CSV role names')
    output = scratch(args.output)
    output.mkdir(parents=True, exist_ok=False)
    source_hash = digest(args.csv) if args.csv else None
    names = ['csv'] if args.csv else ['oracle', 'short', 'long', 'weak']
    for name in names:
        if args.csv:
            columns = csv_columns(args.csv, args.outcome, args.worker, args.firm, args.regressors)
        else:
            sizes = {'oracle': (50, 20, 12), 'short': (50000, 500, 2),
                     'long': (50000, 3000, 20), 'weak': (5000, 500, 20)}
            columns = synthetic('long' if name == 'oracle' else name, *sizes[name])
        path = output / f'{name}.case'
        meta = graph_summary(columns)
        meta.update({'schema': 1, 'case_sha256': None, 'source_sha256': source_hash,
                     'outcome_transformation': 'none', 'selection': 'all supplied rows',
                     'degree_scope': 'input graph before estimator pruning',
                     'generator_sha256': digest(args.generator.resolve()), 'numpy_version': np.__version__,
                     'tolerance': args.tolerance, 'vce': 'robust'})
        if name == 'oracle':
            meta['oracle'] = dense_oracle(columns)
        meta['outputs'] = write_case(args.generator.resolve(), path, columns, args.tolerance)
        meta['case_sha256'] = digest(path)
        path.with_suffix('.json').write_text(json.dumps(meta, indent=2) + '\n')
        print(json.dumps({key: value for key, value in meta.items() if key != 'oracle'}), flush=True)
    if args.csv and digest(args.csv) != source_hash:
        raise ValueError('source CSV changed while preparing inputs')


if __name__ == '__main__':
    main()
