"""Grounded explicit-dummy oracle for the minimum registered weak graph.

SciPy SuperLU solves the full sparse dummy normal equations, independently of
fereg's projector and the separate Schur graph oracle. Frequency checks physically
expand observations and repeat fitting and inference on that expanded design.
"""
import itertools
import json
from pathlib import Path

import numpy as np
import pandas as pd
from scipy.linalg import lstsq
from scipy.sparse import coo_matrix
from scipy.sparse.csgraph import connected_components
from scipy.sparse.linalg import splu


def fit(frame, k, kind, vce):
    n = len(frame)
    y = frame.y.to_numpy()
    X = frame[[f'x{j}' for j in range(1, k + 1)]].to_numpy()
    _, left = np.unique(frame.g1, return_inverse=True)
    _, right = np.unique(frame.g2, return_inverse=True)
    nl, nr = int(left.max()) + 1, int(right.max()) + 1
    graph = coo_matrix((np.ones(n), (left, nl + right)), shape=(nl + nr, nl + nr)).tocsr()
    components, labels = connected_components(graph, directed=False)
    raw = np.ones(n) if kind == 'none' else frame.w.to_numpy()
    assert np.isfinite(raw).all() and (raw > 0).all()
    if kind == 'fw': np.testing.assert_array_equal(raw, raw.astype(np.int64))
    w = raw if kind in ('none', 'fw') else raw * n / raw.sum()
    effective_n = int(raw.sum()) if kind == 'fw' else n
    D = coo_matrix((np.ones(2 * n), (np.tile(np.arange(n), 2), np.r_[left, nl + right])), shape=(n, nl + nr)).tocsr()
    keep = np.ones(nl + nr, dtype=bool)
    for component in range(components):
        keep[nl + np.flatnonzero(labels[nl:] == component)[0]] = False
    grounded = D[:, keep]
    factor = splu((grounded.T @ grounded.multiply(w[:, None])).tocsc())
    Y = np.column_stack([y, X])
    alpha = factor.solve(grounded.T @ (w[:, None] * Y))
    within = Y - grounded @ alpha
    group_mass = np.asarray(D.T @ w).ravel()
    history = []
    for _ in range(5):
        moments = D.T @ (w[:, None] * within)
        error = float(np.max(np.abs(moments / group_mass[:, None])))
        history.append(error)
        if error <= 1e-11: break
        correction = factor.solve(grounded.T @ (w[:, None] * within))
        alpha += correction
        within -= grounded @ correction
    assert history[-1] <= 1e-11, history
    root = np.sqrt(w)
    beta, _, rank, _ = lstsq(within[:, 1:] * root[:, None], within[:, 0] * root, lapack_driver='gelsd')
    assert rank == k
    full_rank = int(keep.sum()) + k
    assert effective_n > full_rank
    residual = within[:, 0] - within[:, 1:] @ beta
    bread = np.linalg.inv(within[:, 1:].T @ (w[:, None] * within[:, 1:]))
    influence = (within[:, 1:] @ bread) * residual[:, None]
    rss = float(residual @ (w * residual))
    if not vce:
        covariance = bread * rss / (effective_n - full_rank)
        correction_df = effective_n - full_rank
    elif vce == 'robust':
        scores = influence * (root if kind == 'fw' else w)[:, None]
        covariance = scores.T @ scores * effective_n / (effective_n - full_rank)
        correction_df = effective_n - full_rank
    else:
        names = vce.split()[1:]
        covariance = np.zeros((k, k))
        for count in range(1, len(names) + 1):
            for subset in itertools.combinations(names, count):
                _, codes = np.unique(frame[list(subset)].to_numpy(), axis=0, return_inverse=True)
                sums = np.zeros((int(codes.max()) + 1, k))
                np.add.at(sums, codes, influence * w[:, None])
                covariance += (-1)**(count + 1) * (sums.T @ sums)
        nonnested = [name for name in ('g1', 'g2')
                     if not any(frame.groupby(name)[cluster].nunique().max() == 1 for cluster in names)]
        nuisance_rank = int(keep.sum()) if len(nonnested) == 2 else frame[nonnested[0]].nunique() if nonnested else 1
        correction_df = effective_n - k - nuisance_rank
        g = min(frame[name].nunique() for name in names)
        covariance *= (effective_n - 1) / correction_df * g / (g - 1)
        eig, vectors = np.linalg.eigh(covariance)
        if eig.min() < 0: covariance = (vectors * np.maximum(eig, 0)) @ vectors.T
    coefficients = np.zeros(nl + nr)
    coefficients[keep] = alpha[:, 0] - alpha[:, 1:] @ beta
    a, b = coefficients[:nl][left], coefficients[nl:][right]
    constant = np.average(a + b, weights=w)
    for component in range(components):
        active = labels[left] == component
        center = np.average(b[active], weights=w[active])
        b[active] -= center
        a[active] += center
    a -= constant
    return dict(beta=beta, covariance=covariance, rss=rss, n=effective_n, rank=full_rank,
        correction_df=correction_df, components=components, refinement_group_means=history,
        outputs=np.array([a, b, residual, X @ beta + constant, y - residual]))


