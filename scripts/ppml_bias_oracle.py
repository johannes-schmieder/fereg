"""Independent explicit-design PPML mean-correction oracle (test use only).

No native estimator, correction helper, or production numerical code is called.
Dense matrices here deliberately make the defining contractions inspectable.
"""
from __future__ import annotations

import numpy as np
from scipy.linalg import solve


def fit_poisson(x, y, weight, exposure, tolerance=2e-13):
    x, y, weight, exposure = map(np.asarray, (x, y, weight, exposure))
    beta = np.zeros(x.shape[1])
    for iteration in range(100):
        mu = exposure * np.exp(x @ beta)
        score = x.T @ (weight * (y - mu))
        hessian = x.T @ ((weight * mu)[:, None] * x)
        step = solve(hessian, score, assume_a="pos")
        if np.max(np.abs(step)) < tolerance:
            beta += step
            break
        rate = 1.0
        for _ in range(60):
            delta = x @ (rate * step)
            with np.errstate(over="ignore", invalid="ignore"):
                gain = np.sum(weight * ((y - mu) * delta - mu * (np.expm1(delta) - delta)))
            if np.isfinite(gain) and gain >= -1e-13:
                break
            rate *= .5
        else:
            raise ValueError("independent PPML line search failed")
        beta += rate * step
    else:
        raise ValueError("independent PPML did not converge")
    mu = exposure * np.exp(x @ beta)
    hessian = x.T @ ((weight * mu)[:, None] * x)
    score = x.T @ (weight * (y - mu))
    if np.max(np.abs(solve(hessian, score, assume_a="pos"))) > 1e-10:
        raise ValueError("independent optimum failed Newton-residual check")
    return beta, mu, iteration + 1


def correction(x, y, weight, mu, unit):
    x, y, weight, mu, unit = map(np.asarray, (x, y, weight, mu, unit))
    hessian = x.T @ ((weight * mu)[:, None] * x)
    inverse = solve(hessian, np.eye(x.shape[1]), assume_a="pos")
    meat = np.zeros_like(hessian)
    derivative_rhs = np.zeros(x.shape[1])
    for group in np.unique(unit):
        use = unit == group
        score = x[use].T @ (weight[use] * (y[use] - mu[use]))
        ji = x[use].T @ ((weight[use] * mu[use])[:, None] * x[use])
        meat += np.outer(score, score)
        derivative_rhs += ji @ inverse @ score
    covariance = inverse @ meat @ inverse
    row_variance = np.einsum("ij,jk,ik->i", x, covariance, x)
    derivative = inverse @ derivative_rhs
    curvature = .5 * inverse @ (x.T @ (weight * mu * row_variance))
    return derivative, curvature


def support_summary(fe, unit, information):
    """Independent per-level table, then nearest-rank public summaries."""
    fe, unit, information = map(np.asarray, (fe, unit, information))
    counts, effective, shares = [], [], []
    for level in np.unique(fe):
        keep = fe == level
        ids, inverse = np.unique(unit[keep], return_inverse=True)
        mass = np.bincount(inverse, weights=information[keep])
        share = mass / mass.sum()
        counts.append(len(ids))
        effective.append(1. / np.sum(share ** 2))
        shares.append(share.max())
    counts = np.asarray(counts)
    return np.r_[len(counts), np.quantile(counts, [0, .25, .5, .75, 1], method="inverted_cdf"),
                 np.sum(counts == 1), np.mean(counts == 1),
                 np.min(effective), np.quantile(effective, .5, method="inverted_cdf"), max(shares)]


def two_fe_design(data):
    first = np.sort(data.sb_sworker.unique())
    second = np.sort(data.sb_sfirm.unique())
    x = np.column_stack(
        [(data.sb_sworker == value).to_numpy(float) for value in first]
        + [(data.sb_sfirm == value).to_numpy(float) for value in second[1:]]
        + [data.x.to_numpy(), data.z.to_numpy()]
    )
    return x, first, second


def philox_signs(units, first, count, seed, stream):
    """Independent vectorized Philox4x32-10, returning a bounded unit/probe tile."""
    units = np.asarray(units, dtype=np.float64).copy()
    units[units == 0.] = 0.
    bits = units.view(np.uint64)[:, None]
    probes = np.arange(first, first + count, dtype=np.uint64)[None, :]
    mask = np.uint64(0xffffffff)
    c0, c1, c2, c3 = np.broadcast_arrays(bits & mask, bits >> np.uint64(32),
                                        probes & mask, probes >> np.uint64(32))
    k0, k1 = int(seed), int(stream)
    for round_ in range(10):
        p0 = np.uint64(0xD2511F53) * c0
        p1 = np.uint64(0xCD9E8D57) * c2
        c0, c1, c2, c3 = ((p1 >> np.uint64(32)) ^ c1 ^ np.uint64(k0), p1 & mask,
                          (p0 >> np.uint64(32)) ^ c3 ^ np.uint64(k1), p0 & mask)
        if round_ != 9:
            k0 = (k0 + 0x9E3779B9) & 0xffffffff
            k1 = (k1 + 0xBB67AE85) & 0xffffffff
    return (2 * (c0 & np.uint64(1)).astype(np.int8) - 1)


def randomized_correction(x, y, weight, mu, unit, probes, seed, stream, batch=1):
    """Explicit J solve with independent RHS tiles and grouped MC covariance."""
    from scipy.linalg import cho_factor, cho_solve
    x, y, weight, mu, unit = map(np.asarray, (x, y, weight, mu, unit))
    unique, codes = np.unique(unit, return_inverse=True)
    h = weight * mu
    factor = cho_factor(x.T @ (h[:, None] * x))
    groups = min(8, probes)
    estimates, sizes = [], []
    first = 0
    for group in range(groups):
        count = probes // groups + int(group < probes % groups)
        fd = np.zeros(len(y)); fc = fd.copy()
        for start in range(first, first + count, batch):
            signs = philox_signs(unique, start, min(batch, first + count - start), seed, stream)[codes]
            rhs = x.T @ ((weight * (y-mu))[:, None] * signs)
            z = x @ cho_solve(factor, rhs)
            fd += np.sum(signs*z, axis=1)
            fc += .5*np.sum(z*z, axis=1)
        estimates.append(cho_solve(factor, x.T @ (h[:, None] * np.column_stack([fd, fc]) / count)))
        sizes.append(count);first += count
    estimates = np.asarray(estimates);sizes = np.asarray(sizes)
    mean = np.average(estimates,axis=0,weights=sizes)
    residual = estimates - mean
    covariance = np.einsum('g,gpi,gpj->pij', sizes, residual, residual) / (probes*(groups-1))
    return mean[:, 0], mean[:, 1], covariance
