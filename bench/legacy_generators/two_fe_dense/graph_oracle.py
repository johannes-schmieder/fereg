"""Independent grounded sparse-direct OLS audit for two-intercept graph screens.

This diagnostic constructs weighted dummy cross-products in SciPy, eliminates
one dummy block, grounds each connected component and uses SuperLU with fresh
residual refinement. It consumes canonical shared input, never native FE state.
The small dense-SVD oracle in run.py provides a separate implementation check.
"""
import argparse
import hashlib
import itertools
import json
from pathlib import Path

import numpy as np
import pandas as pd
from scipy.linalg import lstsq
from scipy.sparse import coo_matrix, diags
from scipy.sparse.csgraph import connected_components
from scipy.sparse.linalg import splu


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def solve(data, kind, vce=''):
    info = json.loads(data.with_suffix('.json').read_text())
    assert sha(data) == info['sha256']
    n, k = info['n'], info['k']
    matrix = np.memmap(data.with_suffix('.bin'), dtype='<f8', mode='r', shape=(len(info['columns']), n))
    columns = {name: matrix[j] for j, name in enumerate(info['columns'])}
    for name, values in columns.items():
        assert hashlib.sha256(values.tobytes()).hexdigest() == info['column_sha256'][name]
    _, left = np.unique(columns['g1'], return_inverse=True)
    _, right = np.unique(columns['g2'], return_inverse=True)
    nl, nr = int(left.max()) + 1, int(right.max()) + 1
    # This full-sample oracle deliberately fails on sample/domain changes.
    raw = np.ones(n) if kind == 'none' else columns['w']
    assert np.isfinite(raw).all() and (raw > 0).all()
    weight = raw if kind in ('none', 'fw') else raw * n / raw.sum()
    effective_n = int(raw.sum()) if kind == 'fw' else n
    if kind == 'fw':
        np.testing.assert_array_equal(raw, raw.astype(np.int64))
    C = coo_matrix((weight, (left, right)), shape=(nl, nr)).tocsr()
    adjacency = coo_matrix((np.ones(n), (left, nl + right)), shape=(nl + nr, nl + nr)).tocsr()
    components, labels = connected_components(adjacency, directed=False)
    assert components == info['components']
    dl, dr = np.bincount(left, weights=weight), np.bincount(right, weights=weight)
    left_scale = diags(1 / dl)
    S = (diags(dr) - C.T @ left_scale @ C).tocsc()
    anchors = np.array([np.flatnonzero(labels[nl:] == component)[0] for component in np.unique(labels)])
    keep = np.ones(nr, dtype=bool)
    keep[anchors] = False
    factor = splu(S[keep][:, keep])
    Y = np.column_stack([columns['y']] + [columns[f'x{j}'] for j in range(1, k + 1)])
    def group_sums(codes, values):
        return np.column_stack([np.bincount(codes, weights=weight * values[:, j]) for j in range(values.shape[1])])
    def project(values):
        ls = group_sums(left, values)
        rs = group_sums(right, values) - C.T @ (ls / dl[:, None])
        rc = np.zeros((nr, values.shape[1]))
        rc[keep] = factor.solve(rs[keep])
        lc = (ls - C @ rc) / dl[:, None]
        return values - lc[left] - rc[right]
    within = project(Y)
    history = []
    for refinement in range(4):
        means = max(float(np.max(np.abs(group_sums(left, within) / dl[:, None]))),
                    float(np.max(np.abs(group_sums(right, within) / dr[:, None]))))
        history.append(means)
        if means <= 1e-11:
            break
        within = project(within)
    assert history[-1] <= 1e-11, history
    root_w = np.sqrt(weight)
    beta, _, rank, _ = lstsq(within[:, 1:] * root_w[:, None], within[:, 0] * root_w, lapack_driver='gelsd')
    assert rank == k
    residual = within[:, 0] - within[:, 1:] @ beta
    rss = float(residual @ (weight * residual))
    total_rank = nl + nr - components + k
    assert effective_n > total_rank
    bread = np.linalg.inv(within[:, 1:].T @ (within[:, 1:] * weight[:, None]))
    correction_df = effective_n - total_rank
    if not vce:
        covariance = bread * rss / correction_df
    else:
        influence = (within[:, 1:] @ bread) * residual[:, None]
        if vce == 'robust':
            scores = influence * (root_w if kind == 'fw' else weight)[:, None]
            covariance = scores.T @ scores * effective_n / correction_df
        else:
            assert vce.startswith('cluster ')
            names = vce.split()[1:]
            covariance = np.zeros((k, k))
            for count in range(1, len(names) + 1):
                for subset in itertools.combinations(names, count):
                    _, codes = np.unique(np.column_stack([columns[name] for name in subset]), axis=0, return_inverse=True)
                    sums = np.column_stack([np.bincount(codes, weights=weight * influence[:, j]) for j in range(k)])
                    covariance += (-1) ** (count + 1) * (sums.T @ sums)
            nonnested = []
            for effect, levels in [('g1', nl), ('g2', nr)]:
                nested = any(pd.DataFrame({'effect': columns[effect], 'cluster': columns[name]})
                             .groupby('effect')['cluster'].nunique().max() == 1 for name in names)
                if not nested: nonnested.append(levels)
            nuisance_rank = nl + nr - components if len(nonnested) == 2 else nonnested[0] if nonnested else 1
            correction_df = effective_n - k - nuisance_rank
            g = min(len(np.unique(columns[name])) for name in names)
            assert correction_df > 0 and g > 1
            covariance *= (effective_n - 1) / correction_df * g / (g - 1)
            eig, vectors = np.linalg.eigh(covariance)
            if eig.size and eig.min() < 0: covariance = (vectors * np.maximum(eig, 0)) @ vectors.T
    finite_score = float(np.max(np.abs(within[:, 1:].T @ (weight * residual)), initial=0) / weight.sum())
    assert finite_score < 1e-11
    return dict(beta=beta, covariance=covariance, rss=rss, n=effective_n, rank=total_rank,
                vce=vce, correction_df=correction_df,
                components=components, refinement_group_means=history, finite_score=finite_score,
                direct_factor_nonzeros=int(factor.L.nnz + factor.U.nnz))


def compare(result_path, oracle):
    run = result_path.parent
    result = json.loads(result_path.read_text())
    assert result['status'] == 'pass' and result['cell']['vce'] == oracle['vce']
    names = [f'x{j}' for j in range(1, result['cell']['k'] + 1)]
    b = pd.read_csv(run / 'coefficients.csv').set_index('name').loc[names, 'value'].to_numpy(dtype=float)
    V = pd.read_csv(run / 'covariance.csv').pivot(index='row', columns='column', values='value').loc[names, names].to_numpy(dtype=float)
    checks = dict(coefficients=bool(np.allclose(b, oracle['beta'], rtol=1e-7, atol=1e-10)),
                  covariance=bool(np.allclose(V, oracle['covariance'], rtol=1e-6, atol=1e-10)),
                  rss=bool(np.isclose(result['metadata']['rss'], oracle['rss'], rtol=1e-8, atol=0)),
                  effective_n=result['metadata']['N'] == oracle['n'])
    if 'fereg_df_correction' in result['metadata']:
        checks['correction_df'] = result['metadata']['fereg_df_correction'] == oracle['correction_df']
    return dict(run=run.name, result_sha256=sha(result_path), input_sha256=result['input_sha256'],
                checks=checks, status='pass' if all(checks.values()) else 'mismatch',
                coefficients_max_error=float(np.max(np.abs(b - oracle['beta']), initial=0)),
                covariance_max_error=float(np.max(np.abs(V - oracle['covariance']), initial=0)),
                rss_relative_error=abs(result['metadata']['rss'] / oracle['rss'] - 1),
                oracle={key: value.tolist() if isinstance(value, np.ndarray) else value for key, value in oracle.items()})


def audit(paths, output):
    assert not output.exists(), output
    cache, records = {}, []
    for path in paths:
        result = json.loads(path.read_text())
        request = json.loads((path.parent / 'request.json').read_text())
        data = Path(request['data'])
        key = (data, result['cell'].get('weight_kind', 'none'), result['cell']['vce'])
        if key not in cache:
            cache[key] = solve(*key)
        record = compare(path, cache[key])
        records.append(record)
        print(record['run'], record['status'], record['coefficients_max_error'], flush=True)
    output.write_text(json.dumps(dict(schema='two-fe-sparse-direct-oracle-v1', source_sha256=sha(Path(__file__)), records=records), indent=2, sort_keys=True) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('results', type=Path, nargs='+')
    args = parser.parse_args()
    audit(args.results, args.output)
