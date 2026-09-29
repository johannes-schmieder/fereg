"""Independent bounded explicit-dummy OLS/Poisson check (SciPy, no fereg code)."""
import argparse
import math
from pathlib import Path

import numpy as np
import pandas as pd
from scipy import sparse
from scipy.optimize import linprog
from scipy.sparse.linalg import lsmr
from scipy.sparse.csgraph import connected_components
from scipy.special import gammaln, xlogy

from common import HERE, read_json, write_json


def dummy_matrix(frame, effects, slopes=None):
    matrices, codes, widths = [], [], []
    for name in effects:
        code, levels = pd.factorize(frame[name], sort=True)
        width = len(levels)
        indicator = sparse.csr_matrix((np.ones(len(code)), (np.arange(len(code)), code)), shape=(len(code), width))
        matrices.append(indicator)
        matrices.extend(indicator.multiply(frame[z].to_numpy()[:, None]) for z in (slopes or {}).get(name, []))
        codes.append(code)
        widths.append(width)
    F = sparse.hstack(matrices, format="csr")
    if slopes:
        if F.shape[1] > 1200:
            raise ValueError('Slope explicit rank audit exceeds bounded oracle width')
        rank = int(np.linalg.matrix_rank(F.toarray()))
    elif len(effects) == 1:
        rank = widths[0]
    elif len(effects) == 2:
        left, right = codes[0], codes[1] + widths[0]
        G = sparse.coo_matrix((np.ones(2 * len(left)), (np.r_[left, right], np.r_[right, left])), shape=(sum(widths), sum(widths))).tocsr()
        rank = sum(widths) - connected_components(G, directed=False, return_labels=False)
    else:
        if F.shape[1] > 1200:
            raise ValueError("three-way explicit rank audit exceeds bounded oracle width")
        rank = int(np.linalg.matrix_rank(F.toarray()))
    return F, rank


def solve(A, b):
    # Controls and small FE indicators have very different column norms.
    # Without equilibration, LSMR's normwise stopping test can leave individual
    # FE scores above the unchanged outer certificate even at a stable fit.
    A = A.tocsr()
    norms = np.sqrt(np.asarray(A.power(2).sum(axis=0)).ravel())
    if not np.isfinite(norms).all() or np.any(norms <= 0):
        raise ValueError("invalid explicit-design column norm")
    scaling = 1 / norms
    solution = lsmr(A.multiply(scaling), b, atol=1e-13, btol=1e-13, maxiter=20000)
    if solution[1] not in (0, 1, 2, 4, 5):
        raise ValueError(f"independent sparse solve did not converge: {solution[1]}")
    return scaling * solution[0]


def separation_sample(frame, cell):
    """Explicit full-model recession cone, independent of native detectors."""
    trace = []
    for _ in range(len(frame)+1):
        while True:
            keep = np.ones(len(frame), dtype=bool)
            for effect in cell['effects']:
                grouped = frame.groupby(effect).y
                keep &= (grouped.transform('sum').to_numpy() > 0) & (grouped.transform('size').to_numpy() > 1)
            if keep.all():break
            frame = frame.loc[keep].reset_index(drop=True)
        if frame.empty or not (frame.y > 0).any():
            raise ValueError('No positive-outcome oracle sample remains')
        F, _ = dummy_matrix(frame, cell['effects'], cell.get('slopes'))
        X = frame[[f'x{j}' for j in range(1,cell['k']+1)]].to_numpy()
        D = sparse.hstack([sparse.csr_matrix(X), F], format='csr')
        positive = frame.y.to_numpy() > 0
        if positive.all():break
        cost = np.asarray(D[~positive].sum(axis=0)).ravel()
        result = linprog(cost, A_ub=D[~positive], b_ub=np.zeros(sum(~positive)),
                         A_eq=D[positive], b_eq=np.zeros(sum(positive)), bounds=(-1,1),
                         method='highs', options={'primal_feasibility_tolerance':1e-9,
                                                  'dual_feasibility_tolerance':1e-9})
        if not result.success:
            raise ValueError('Explicit recession-cone LP failed: '+result.message)
        direction = np.asarray(D @ result.x)
        scale = max(1., float(np.max(abs(direction))))
        interior = float(np.max(abs(direction[positive])))
        violation = float(np.max(direction[~positive]))
        if not np.isfinite(direction).all() or interior > 1e-9*scale or violation > 1e-9*scale:
            raise ValueError('Independent recession-direction sign certificate failed')
        remove = (~positive) & (direction < -1e-8*scale)
        trace.append(dict(rows=len(frame), objective=float(result.fun),
                          removed=frame.obs_id.to_numpy()[remove].tolist(),
                          maximum_interior_residual=interior, maximum_boundary_violation=violation))
        if not remove.any():
            if result.fun < -1e-8*scale:
                raise ValueError('Recession LP is negative without a certified deletion margin')
            break
        frame = frame.loc[~remove].reset_index(drop=True)
    else:
        raise ValueError('Independent separation fixed point did not finish')
    return frame, trace


def oracle(path, cell):
    frame = pd.read_stata(path, convert_categoricals=False)
    if len(frame) > 7680:
        raise ValueError('oracle is bounded to 7680 observations')
    separation = []
    if cell['section'] == 'zeros':
        frame, separation = separation_sample(frame, cell)
    n, k = len(frame), cell["k"]
    if n > 7680:
        raise ValueError("oracle is bounded to 7680 observations")
    F, rank = dummy_matrix(frame, cell["effects"], cell.get('slopes'))
    X = frame[[f"x{i}" for i in range(1, k + 1)]].to_numpy()
    y = frame.y.to_numpy()
    if cell["model"] == "ols":
        Z = np.column_stack([X[:, j] - F @ solve(F, X[:, j]) for j in range(k)])
        y_within = y - F @ solve(F, y)
        b = np.linalg.lstsq(Z, y_within, rcond=None)[0]
        residual = y_within - Z @ b
        rss = float(residual @ residual)
        V = np.linalg.inv(Z.T @ Z) * rss / (n - rank - k)
        extra = {"rss": rss, "df_r": n - rank - k}
    else:
        D = sparse.hstack([sparse.csr_matrix(X), F], format="csr")
        eta = np.full(n, np.log(y.mean()))
        b_all = np.zeros(D.shape[1])
        for iteration in range(100):
            mu = np.exp(eta)
            root = np.sqrt(mu)
            # Solve for a Newton increment: an absolute working-response solve
            # loses score accuracy when a large FE predictor is nearly optimal.
            increment = solve(D.multiply(root[:, None]), (y - mu) / root)
            direction = D @ increment
            objective = np.sum(mu - y * eta)
            step = 1.0
            while step > 2**-30:
                trial = eta + step * direction
                if np.isfinite(trial).all() and np.max(trial) < 700 and np.sum(np.exp(trial) - y * trial) <= objective + 1e-11 * max(1, abs(objective)):
                    break
                step *= .5
            if step <= 2**-30:
                raise ValueError("independent Poisson line search failed")
            eta = trial
            # Recover the same predictor in the explicit model coordinates.
            b_all = solve(D, eta)
            fitted = np.exp(eta)
            score = np.asarray(D.T @ (y - fitted)).ravel()
            scale = np.maximum(1, np.asarray(abs(D).T @ (y + fitted)).ravel())
            if np.max(abs(score) / scale) < 1e-11:
                break
        else:
            raise ValueError("independent PPML iteration cap")
        mu = np.exp(eta)
        root = np.sqrt(mu)
        WF = F.multiply(root[:, None])
        Z = np.column_stack([X[:, j] - F @ solve(WF, root * X[:, j]) for j in range(k)])
        bread = np.linalg.inv(Z.T @ (mu[:, None] * Z))
        meat = Z.T @ ((y - mu)[:, None] ** 2 * Z)
        V = bread @ meat @ bread * n / (n - 1)
        b = b_all[:k]
        score = np.asarray(D.T @ (y - mu)).ravel()
        scale = np.maximum(1, np.asarray(abs(D).T @ (y + mu)).ravel())
        if np.max(abs(score) / scale) > 1e-9:
            raise ValueError("independent Poisson score certificate failed")
        extra = {"ll": float(np.sum(y * eta - mu - gammaln(y + 1))),
                 "deviance": float(2 * np.sum(xlogy(y, y / mu) - y + mu)),
                 "maximum_scaled_score": float(np.max(abs(score) / scale)), "iterations": iteration + 1}
    return {"schema": "systematic-explicit-dummy-oracle-v2", "status": "pass", "N": n,
            "sample_sha256": __import__('hashlib').sha256(b''.join(int(x).to_bytes(8,'little') for x in frame.obs_id)).hexdigest(),
            "linear_solver": "column-equilibrated SciPy LSMR, atol=btol=1e-13",
            "separation_trace": separation, "fe_rank": rank, "coefficient_names": [f"x{i}" for i in range(1, k + 1)],
            "coefficients": b.tolist(), "covariance": V.tolist(), **extra}


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("data", type=Path)
    p.add_argument("cell", type=Path)
    p.add_argument("output", type=Path)
    a = p.parse_args()
    write_json(a.output, oracle(a.data, read_json(a.cell)))
