"""Independent diagnostic WLS oracle for two categorical FE partitions.

Eliminate the first partition analytically, ground one vertex per connected
component of the remaining graph, and factor that sparse system with SuperLU.
No fereg/fixest absorption, stopping rule, rank policy or output is used. This
is a bounded scientific oracle, not a production route or timing comparator.
"""
import numpy as np
from scipy import sparse
from scipy.sparse.csgraph import connected_components
from scipy.sparse.linalg import splu


def group_sum(ids, values, size):
    if values.ndim == 1:
        return np.bincount(ids, weights=values, minlength=size)
    return np.column_stack([np.bincount(ids, weights=v, minlength=size)
                            for v in values.T])


def relative_scores(X, g, h, weights, residual):
    """Original-row normal equations, normalized separately for every column."""
    score = weights * residual
    residual_norm = np.sqrt(np.dot(score, residual))
    if residual_norm == 0:
        return dict(finite=0.0, first_fe=0.0, second_fe=0.0)
    result = {}
    if X.shape[1]:
        norms = np.sqrt(np.sum(weights[:, None] * X**2, axis=0))
        result['finite'] = float(np.max(np.abs(X.T @ score) / norms) / residual_norm)
    else:
        result['finite'] = 0.0
    for name, ids in [('first_fe', g), ('second_fe', h)]:
        norms = np.sqrt(np.bincount(ids, weights=weights))
        result[name] = float(np.max(np.abs(np.bincount(ids, weights=score)) / norms)
                             / residual_norm)
    return result


def solve(y, X, first, second, weights=None):
    y, X = np.asarray(y, dtype=float), np.asarray(X, dtype=float)
    w = np.ones(len(y)) if weights is None else np.asarray(weights, dtype=float)
    if X.ndim != 2 or X.shape[0] != len(y) or w.shape != y.shape:
        raise ValueError('inconsistent oracle dimensions')
    if not all(np.all(np.isfinite(v)) for v in (y, X, w)) or np.any(w <= 0):
        raise ValueError('oracle requires finite rows and strictly positive weights')
    _, g = np.unique(first, return_inverse=True)
    _, h = np.unique(second, return_inverse=True)
    ng, nh = int(g.max()) + 1, int(h.max()) + 1
    swg, swh = np.bincount(g, weights=w), np.bincount(h, weights=w)
    incidence = sparse.coo_matrix((w, (g, h)), shape=(ng, nh)).tocsr()
    cross = incidence.T @ sparse.diags(1 / swg) @ incidence
    components, labels = connected_components(cross, directed=False)
    _, roots = np.unique(labels, return_index=True)
    active = np.ones(nh, dtype=bool)
    active[roots] = False
    laplacian = (sparse.diags(swh) - cross).tocsc()
    grounded = laplacian[active][:, active].tocsc()
    Z = np.column_stack([y, X])
    centered = Z - (group_sum(g, w[:, None] * Z, ng) / swg[:, None])[g]
    rhs = group_sum(h, w[:, None] * centered, nh)
    gamma = np.zeros((nh, Z.shape[1]))
    lu_nnz = 0
    backward_error = 0.0
    if active.any():
        factor = splu(grounded)
        gamma[active] = factor.solve(rhs[active])
        # One direct iterative-refinement correction, not iterative absorption.
        gamma[active] += factor.solve(rhs[active] - grounded @ gamma[active])
        lu_nnz = factor.L.nnz + factor.U.nnz
        error = np.abs(grounded @ gamma[active] - rhs[active])
        scale = abs(grounded) @ abs(gamma[active]) + abs(rhs[active])
        backward_error = float(np.max(error / np.maximum(scale, np.finfo(float).tiny)))
    alpha = group_sum(g, w[:, None] * (Z - gamma[h]), ng) / swg[:, None]
    projected = Z - alpha[g] - gamma[h]
    Xp, yp = projected[:, 1:], projected[:, 0]
    gram = Xp.T @ (w[:, None] * Xp)
    if X.shape[1]:
        eigenvalues = np.linalg.eigvalsh(gram)
        if eigenvalues[0] <= max(len(y), X.shape[1]) * np.finfo(float).eps * eigenvalues[-1]:
            raise ValueError('oracle finite design is rank deficient or ambiguous')
        beta = np.linalg.solve(gram, Xp.T @ (w * yp))
        bread = np.linalg.inv(gram)
        condition = float(eigenvalues[-1] / eigenvalues[0])
    else:
        beta, bread, condition = np.empty(0), np.empty((0, 0)), None
    first_coef = alpha[:, 0] - alpha[:, 1:] @ beta
    second_coef = gamma[:, 0] - gamma[:, 1:] @ beta
    residual = y - X @ beta - first_coef[g] - second_coef[h]
    rss = float(np.dot(w * residual, residual))
    rank = ng + nh - components + X.shape[1]
    if rank >= len(y):
        raise ValueError('oracle requires positive residual degrees of freedom')
    scores = relative_scores(X, g, h, w, residual)
    projection_score = max(max(relative_scores(np.empty((len(y), 0)), g, h, w, col).values())
                           for col in projected.T)
    reconstruction_error = float(np.max(np.abs(residual - (yp - Xp @ beta))))
    assert max(scores.values()) < 1e-10 and projection_score < 1e-10
    assert backward_error < 1e-12
    return dict(coefficient=beta, bread=bread, covariance=bread * rss / (len(y) - rank),
                residual=residual, projected_X=Xp, rank=int(rank), rss=rss,
                certificate=dict(components=int(components), first_levels=ng, second_levels=nh,
                                 grounded_dimension=int(active.sum()), grounded_nnz=grounded.nnz,
                                 factor_nnz=int(lu_nnz), finite_condition=condition,
                                 maximum_relative_scores=scores,
                                 maximum_projection_score=projection_score,
                                 factor_backward_error=backward_error,
                                 reconstruction_error=reconstruction_error))
