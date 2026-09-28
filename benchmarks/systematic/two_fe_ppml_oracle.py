"""Bounded positive-outcome PPML proposal from a grounded full sparse design.

This offline diagnostic uses direct full-Hessian Newton solves, not either
estimator's absorption algorithm. An independent outward-rounded certificate
must establish optimum accuracy before adjudicating a coefficient discrepancy.
"""
import numpy as np
from scipy import sparse
from scipy.sparse.csgraph import connected_components
from scipy.sparse.linalg import splu
from scipy.special import gammaln, xlogy


def fit(y, X, first, second):
    y, X = np.asarray(y, dtype=float), np.asarray(X, dtype=float)
    if (X.ndim != 2 or not 8 <= len(y) <= 75000 or X.shape[0] != len(y)
            or not 1 <= X.shape[1] <= 50 or np.any(y <= 0)
            or not np.isfinite(y).all() or not np.isfinite(X).all()):
        raise ValueError('unsupported positive-outcome PPML oracle input')
    _, g = np.unique(first, return_inverse=True)
    _, h = np.unique(second, return_inverse=True)
    n, k = X.shape
    ng, nh = int(g.max()) + 1, int(h.max()) + 1
    if len(g) != n or len(h) != n or nh > 1200:
        raise ValueError('unsupported two-FE oracle topology')
    F = sparse.csr_matrix((np.ones(n), (np.arange(n), g)), shape=(n, ng))
    G = sparse.csr_matrix((np.ones(n), (np.arange(n), h)), shape=(n, nh))
    incidence = F.T @ G
    components, labels = connected_components(incidence.T @ incidence, directed=False)
    _, roots = np.unique(labels, return_index=True)
    keep = np.ones(nh, dtype=bool)
    keep[roots] = False
    D = sparse.hstack([F, G[:, keep], sparse.csr_matrix(X)], format='csr')
    if D.shape[1] >= n:
        raise ValueError('oracle requires positive residual degrees of freedom')
    theta = np.zeros(D.shape[1])
    theta[:ng] = np.log(y.mean())
    trace = []
    for iteration in range(25):
        eta = D @ theta
        mu = np.exp(eta)
        score = np.asarray(D.T @ (y - mu)).ravel()
        H = (D.T @ D.multiply(mu[:, None])).tocsc()
        factor = splu(H, permc_spec='NATURAL')
        step = factor.solve(score)
        direction = D @ step
        maximum = float(np.max(abs(direction)))
        rate = 1.0
        if maximum >= 2e-13:
            for _ in range(40):
                delta = np.asarray(rate * direction, dtype=np.longdouble)
                weights = np.asarray(mu, dtype=np.longdouble)
                gain = np.sum((y.astype(np.longdouble) - weights) * delta
                              - weights * (np.expm1(delta) - delta))
                if np.isfinite(gain) and gain >= 1e-4 * rate * np.dot(score, step):
                    break
                rate *= .5
            else:
                raise ValueError('direct Poisson oracle line search failed')
        theta += rate * step
        trace.append(dict(iteration=iteration + 1, maximum_direction=maximum,
                          step=rate, maximum_score=float(np.max(abs(score))),
                          factor_nnz=int(factor.L.nnz + factor.U.nnz)))
        if maximum < 2e-13:
            break
    else:
        raise ValueError('direct Poisson oracle iteration limit')
    eta, beta = D @ theta, theta[-k:]
    mu = np.exp(eta)
    H = (D.T @ D.multiply(mu[:, None])).tocsc()
    factor = splu(H, permc_spec='NATURAL')
    units = np.zeros((D.shape[1], k))
    units[-k:] = np.eye(k)
    inverse_columns = factor.solve(units)
    # Full-design influence columns independently form the finite sandwich.
    influence = (D @ inverse_columns) * (y - mu)[:, None]
    covariance = influence.T @ influence * n / (n - 1)
    score = np.asarray(D.T @ (y - mu)).ravel()
    norm = np.sqrt(np.asarray(D.power(2).T @ mu).ravel())
    scale = max(np.linalg.norm((y - mu) / np.sqrt(mu)), np.linalg.norm(y / np.sqrt(mu)))
    normalized_score = float(np.max(abs(score) / norm) / scale)
    assert normalized_score < 1e-12
    return dict(beta=beta, mu=mu, covariance=covariance, bread=inverse_columns[-k:],
                rank=int(D.shape[1]), components=int(components), first_levels=ng,
                second_levels=nh, iterations=trace, maximum_relative_score=normalized_score,
                deviance=float(2 * np.sum(xlogy(y, y / mu) - y + mu)),
                log_likelihood=float(np.sum(y * eta - mu - gammaln(y + 1))))
