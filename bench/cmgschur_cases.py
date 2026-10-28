#!/usr/bin/env python3
"""Build reproducible two-FE prototype cases, or convert a local numeric CSV.

Requires NumPy and the existing fereg-oracle replay_large executable. Outputs
are private scratch inputs under the declared CMG-Schur task directory; only aggregate metadata
belongs in retained evidence. CSV rows/outcomes are preserved without selection
or transformation. A small synthetic case includes an independent dense oracle.
"""
from __future__ import annotations

import argparse
from array import array
import csv
import io
import json
from pathlib import Path
import subprocess

import numpy as np

from native_io import ROOT, case_header, digest

HIGH_DEGREE_SIZES = {'sampled-oracle': (60, 40, 50),
                     'd30-mixed': (10000, 1000, 60),
                     'd30-weak': (10000, 1000, 60),
                     'd60-stress': (5000, 1000, 120)}


def scratch(path):
    path = path.resolve()
    if not any(path.is_relative_to((ROOT / 'target/tmp' / task).resolve())
               for task in ['cmgschur', 'cmgschur-opt', 'cmgschur-sampled', 'solver-routing', 'auto-routing']):
        raise ValueError('output must be under a declared CMG/solver-routing/auto-routing task in target/tmp')
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


def additional_case(kind, workers=5000, firms=500, periods=20, seed=20261013):
    """Batching fixtures with paired observations and distinct FE content."""
    if kind not in {'wide', 'orthogonal'} or periods % 2:
        raise ValueError('additional cases require wide/orthogonal and an even period count')
    base = synthetic('long', workers, firms, periods, seed)
    worker = base[-2]
    # Consecutive duplicate links permit exactly cancelling within-link columns.
    firm = np.repeat(base[-1].reshape(workers, periods)[:, :periods//2], 2, axis=1).ravel()
    rng = np.random.default_rng(seed + 1)
    n = len(worker)
    def paired():
        values = rng.integers(-16, 17, n//2).astype(float) / 8
        return np.column_stack([values, -values]).ravel()
    if kind == 'orthogonal':
        x = np.array([paired() for _ in range(4)])
        y = .5*x[0] - .25*x[1] + .75*x[2] - .125*x[3] + paired()/64
    else:
        x = rng.standard_normal((16, n))
        for j in range(16):
            if j % 4 == 0:
                x[j] = paired()
            if j % 4 in {1, 3}:
                x[j] += (1 + j/8) * np.sin(worker * (.017 + j*.003))
            if j % 4 in {2, 3}:
                x[j] += (1 + j/16) * np.cos(firm * (.031 + j*.005))
        coefficients = np.array([(-1.)**j / (j + 1) for j in range(16)])
        y = (coefficients @ x + np.sin(worker*.071) + .6*np.cos(firm*.13)
             + .3*rng.standard_normal(n))
    return [y, *x, worker, firm]


def high_degree_case(kind, workers, firms, periods, seed=20261013):
    """Connected high-degree panels; degree counts distinct firms, not rows."""
    if kind not in HIGH_DEGREE_SIZES:
        raise ValueError('unknown high-degree fixture')
    groups = 10 if kind == 'd30-weak' else 1
    degree = 60 if kind == 'd60-stress' else 30
    if workers < firms + groups + 1 or firms % groups or firms // groups < degree or periods <= degree:
        raise ValueError('fixture requires room for connectivity, distinct employers and repeated links')
    rng = np.random.default_rng(seed)
    worker = np.repeat(np.arange(workers), periods)
    employer = np.empty((workers, periods), dtype=np.int64)
    width = firms // groups
    degrees = rng.choice([1, 2, 4, 6, 12, degree], workers, p=[.10, .10, .15, .15, .20, .30])
    if kind in {'sampled-oracle', 'd60-stress'}:
        degrees[:] = degree
    degrees[-1] = degree
    for i in range(workers):
        group = i % groups
        if i < firms:
            first = i // groups
            visits = np.array([group * width + first, group * width + (first + 1) % width])
        elif i < firms + groups - 1:
            bridge = i - firms
            visits = np.array([bridge * width, (bridge + 1) * width])
        else:
            visits = group * width + rng.choice(width, int(degrees[i]), replace=False)
        employer[i] = np.resize(visits, periods)
    employer = employer.ravel()
    x = rng.standard_normal((2, worker.size))
    x[0] += .4 * np.sin(worker * .07)
    x[1] += .3 * np.cos(employer * .11)
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


def dense_oracle(columns, include_intercept=True):
    """Dense dummy OLS and HC1 covariance; no fereg solver is used."""
    y, *rest = columns
    xs, worker, firm = rest[:-2], rest[-2], rest[-1]
    worker_levels, wi = np.unique(worker, return_inverse=True)
    firm_levels, fi = np.unique(firm, return_inverse=True)
    if include_intercept:
        design = np.column_stack([np.ones(len(y)), *xs,
                                  np.eye(len(worker_levels))[wi, 1:],
                                  np.eye(len(firm_levels))[fi, 1:]])
    else:
        # Full worker dummies retain the nuisance constant without a structural
        # constant column entering the production projection's RHS batch.
        design = np.column_stack([*xs, np.eye(len(worker_levels))[wi],
                                  np.eye(len(firm_levels))[fi, 1:]])
    inverse = np.linalg.pinv(design, rcond=1e-13)
    beta = inverse @ y
    fitted = design @ beta
    residual = y - fitted
    rank = int(np.linalg.matrix_rank(design))
    if rank != design.shape[1] or rank >= len(y):
        raise ValueError('dense fixture must be connected and have full design rank')
    covariance = (inverse * residual) @ (inverse * residual).T * len(y) / (len(y) - rank)
    start = int(include_intercept)
    return {'method': ('NumPy dense dummy SVD and HC1, one firm and worker base' if include_intercept
                       else 'NumPy dense dummy SVD and HC1, full worker dummies and one firm base, no structural constant'),
            'rank': rank, 'regressors': len(xs), 'beta': beta[start:start+len(xs)].tolist(),
            'covariance': covariance[start:start+len(xs), start:start+len(xs)].tolist(),
            'fitted': fitted.tolist(), 'residual': residual.tolist()}


def noconstant_template(arguments, metadata, regressors, matrix_count):
    """Remove the reported constant and resize its five output buffers."""
    if regressors < 1 or arguments.count(b'include_intercept=1') != 1:
        raise ValueError('nocons fixture requires regressors and one canonical intercept option')
    arguments = [b'include_intercept=0' if a == b'include_intercept=1' else a for a in arguments]
    shapes = {'RB': (1, regressors), 'RV': (regressors, regressors),
              'RSTATUS': (regressors, 4), 'RMARKB': (1, regressors),
              'RMARKV': (regressors, regressors)}
    stream, output, found = io.BytesIO(metadata), io.BytesIO(), set()
    for _ in range(matrix_count):
        length = int(stream.readline())
        name = stream.read(length)
        if stream.read(1) != b'\n':
            raise ValueError('invalid canonical matrix name frame')
        rows, cols = map(int, stream.readline().split())
        values = [stream.readline() for _ in range(rows * cols)]
        key = name.decode()
        if key in shapes:
            if any(value != b'0000000000000000\n' for value in values):
                raise ValueError('canonical coefficient output buffers must start at zero')
            rows, cols = shapes[key]
            values = [b'0000000000000000\n'] * (rows * cols)
            found.add(key)
        output.write(str(length).encode() + b'\n' + name + b'\n')
        output.write(f'{rows} {cols}\n'.encode() + b''.join(values))
    if stream.read() or found != set(shapes):
        raise ValueError('unexpected canonical nocons metadata layout')
    return arguments, output.getvalue()


def write_case(generator, path, columns, tolerance=1e-8, include_intercept=True, *, effect_count=2, weights=None,
               vce='robust', observation_outputs=True):
    """Use the canonical Rust serializer for metadata, replace only numeric data.

    Without observation outputs only the mandatory sample destination remains,
    so no residual, xb, xbd or absorbed output is requested."""
    template = path.with_suffix('.template.case')
    if not 1 <= effect_count <= 16 or vce not in {'robust', 'iid'}:
        raise ValueError('native template requires 1..16 categorical effects and robust or IID VCE')
    regressors = len(columns) - effect_count - 1
    template_levels = ','.join(map(str, ([20, 10] + [5] * (effect_count - 2))[:effect_count]))
    subprocess.run([str(generator), '--output', str(template), '--rows', '80',
                    '--levels', template_levels, '--regressors', str(regressors),
                    '--weights', 'unit' if weights is None else 'pw', '--vce', vce, '--graph', 'random-mobility',
                    '--solver', 'auto', '--seed', '20261013', '--tolerance', str(tolerance),
                    *([] if observation_outputs else ['--no-observation-outputs'])],
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
        if not include_intercept:
            arguments, metadata = noconstant_template(arguments, metadata, regressors, shape[3])
        fields = dict(a.decode().split('=', 1) for a in arguments[1:])
        n = len(columns[0])
        if any(len(column) != n for column in columns) or (weights is not None and (len(weights) != n
                or not np.all(np.isfinite(weights)) or np.any(np.asarray(weights) <= 0))):
            raise ValueError('fixture columns and positive finite weights must have equal lengths')
        data = [np.ones(n), *columns, *([] if weights is None else [weights])]
        if len(data) != int(fields['output_sample']) - 1:
            raise ValueError('canonical template input layout changed')
        outputs = ['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed']
        if not observation_outputs:
            if any(fields['output_' + name] != '0' for name in outputs[1:]):
                raise ValueError('sample-only template requests an observation output')
            outputs = outputs[:1]
        for name in outputs:
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
        return {name: int(fields['output_' + name]) - 1 for name in outputs}
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
    parser.add_argument('--extra-cases', nargs='+', choices=['wide', 'orthogonal', *HIGH_DEGREE_SIZES], default=[],
                        help='append optional batching/high-degree fixtures; original cases are unchanged')
    args = parser.parse_args()
    if not 0 < args.tolerance < 1 or (args.csv and not all([args.outcome, args.worker, args.firm])):
        parser.error('require valid tolerance and all three CSV role names')
    if len(set(args.extra_cases)) != len(args.extra_cases) or (args.csv and args.extra_cases):
        parser.error('extra cases must be unique and cannot accompany CSV conversion')
    output = scratch(args.output)
    output.mkdir(parents=True, exist_ok=False)
    source_hash = digest(args.csv) if args.csv else None
    names = ['csv'] if args.csv else ['oracle', 'short', 'long', 'weak', *args.extra_cases]
    for name in names:
        if args.csv:
            columns = csv_columns(args.csv, args.outcome, args.worker, args.firm, args.regressors)
        elif name in {'wide', 'orthogonal'}:
            size = (50, 20, 12) if name == 'orthogonal' else (5000, 500, 20)
            columns = additional_case(name, *size)
        elif name in HIGH_DEGREE_SIZES:
            columns = high_degree_case(name, *HIGH_DEGREE_SIZES[name])
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
        if name in {'oracle', 'orthogonal', 'sampled-oracle'}:
            meta['oracle'] = dense_oracle(columns, include_intercept=name != 'orthogonal')
        if name in HIGH_DEGREE_SIZES:
            meta['fixture'] = name
            meta['graph_seed'] = 20261013
            meta['maximum_worker_degree'] = 60 if name == 'd60-stress' else 30
        if name in {'wide', 'orthogonal'}:
            meta['fixture'] = ('16 controls: four exactly FE-orthogonal, others with worker/firm/both FE content'
                               if name == 'wide' else 'nocons: all four controls and outcome exactly FE-orthogonal by paired cancellation')
        if name == 'orthogonal':
            meta['include_intercept'] = False
        meta['outputs'] = write_case(args.generator.resolve(), path, columns, args.tolerance,
                                     include_intercept=name != 'orthogonal')
        meta['case_sha256'] = digest(path)
        path.with_suffix('.json').write_text(json.dumps(meta, indent=2) + '\n')
        print(json.dumps({key: value for key, value in meta.items() if key != 'oracle'}), flush=True)
    if args.csv and digest(args.csv) != source_hash:
        raise ValueError('source CSV changed while preparing inputs')


if __name__ == '__main__':
    main()
