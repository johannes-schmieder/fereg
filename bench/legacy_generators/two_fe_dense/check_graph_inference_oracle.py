"""Check Schur-oracle inference against full dummy SVD and literal FW expansion."""
import argparse
import hashlib
import itertools
import json
from pathlib import Path

import numpy as np
import pandas as pd
from scipy.linalg import lstsq

from graph_oracle import solve


def check(root):
    root.mkdir(parents=True, exist_ok=False)
    records = []
    for disconnected, kind, reverse in itertools.product([False, True], ['none', 'aw', 'pw', 'fw'], [False, True]):
        n = 600; i = np.arange(n); rng = np.random.default_rng(104729)
        component = i % 2 if disconnected else np.zeros(n, dtype=int)
        left = i // 2 % 17 + component * 17; right = i // 34 % 13 + component * 13
        X = rng.normal(size=(n, 3)); y = X @ np.array([.3, -.1, .07]) + np.sin(left) + np.cos(right) + rng.normal(size=n)
        w = rng.integers(1, 7, n).astype(float) if kind == 'fw' else np.exp(rng.normal(size=n))
        if kind == 'none': w = np.ones(n)
        frame = pd.DataFrame(dict(g1=left, g2=right, y=y, w=w, x1=X[:, 0], x2=X[:, 1], x3=X[:, 2], c1=i % 11, c2=i // 11 % 7))
        if reverse: frame = frame.iloc[::-1].reset_index(drop=True)
        path = root / f'd{int(disconnected)}-{kind}-r{int(reverse)}.dta'
        frame.to_stata(path, write_index=False, version=118)
        raw = {name: np.asarray(frame[name], dtype='<f8').tobytes() for name in frame}
        path.with_suffix('.bin').write_bytes(b''.join(raw.values()))
        path.with_suffix('.json').write_text(json.dumps(dict(n=n, k=3, columns=list(frame), components=2 if disconnected else 1,
            sha256=hashlib.sha256(path.read_bytes()).hexdigest(), column_sha256={name: hashlib.sha256(value).hexdigest() for name, value in raw.items()})))
        # The reference uses all dummy columns and SVD, including the redundant
        # component intercepts. FW rows are physically expanded before fitting.
        reference = frame.iloc[np.repeat(np.arange(n), frame.w.to_numpy().astype(int))].reset_index(drop=True) if kind == 'fw' else frame
        m = len(reference)
        weights = reference.w.to_numpy() if kind in ('aw', 'pw') else np.ones(m)
        if kind in ('aw', 'pw'): weights = weights * m / weights.sum()
        D = [pd.get_dummies(reference[name], dtype=float).to_numpy() for name in ('g1', 'g2')]
        Z = np.column_stack([reference[['x1', 'x2', 'x3']].to_numpy(), *D]); Y = reference.y.to_numpy()
        root_w = np.sqrt(weights); weighted = Z * root_w[:, None]
        # The complete dummy matrix has exactly one redundant direction per
        # component. LAPACK's default cutoff can retain a rounding remnant of
        # that direction; use the same explicit cutoff as the pseudoinverse.
        b, _, rank, _ = lstsq(weighted, Y * root_w, cond=1e-13, lapack_driver='gelsd')
        assert rank == 3 + sum(block.shape[1] for block in D) - (2 if disconnected else 1)
        residual = Y - Z @ b
        inverse = np.linalg.pinv(weighted, rcond=1e-13)
        bread = inverse[:3] @ inverse[:3].T
        influence = inverse[:3].T / root_w[:, None] * residual[:, None]
        rss = float(residual @ (weights * residual))
        for vce in ['', 'robust', 'cluster g1', 'cluster c1', 'cluster c1 c2', 'cluster g1 g2']:
            if kind == 'pw' and not vce: continue
            correction_df = m - rank
            if not vce: covariance = bread * rss / correction_df
            elif vce == 'robust':
                scores = influence * weights[:, None]
                covariance = scores.T @ scores * m / correction_df
            else:
                names = vce.split()[1:]; covariance = np.zeros((3, 3))
                for count in range(1, len(names) + 1):
                    for subset in itertools.combinations(names, count):
                        sums = pd.DataFrame(influence * weights[:, None]).groupby(
                            [reference[name] for name in subset]).sum().to_numpy()
                        covariance += (-1) ** (count + 1) * sums.T @ sums
                nonnested = [block for effect, block in zip(('g1', 'g2'), D)
                             if not any(reference.groupby(effect)[name].nunique().max() == 1 for name in names)]
                nuisance_rank = np.linalg.matrix_rank(np.column_stack(nonnested)) if nonnested else 1
                correction_df = m - 3 - nuisance_rank
                g = min(reference[name].nunique() for name in names)
                covariance *= (m - 1) / correction_df * g / (g - 1)
                eig, vectors = np.linalg.eigh(covariance)
                if eig.min() < 0: covariance = (vectors * np.maximum(eig, 0)) @ vectors.T
            actual = solve(path, kind, vce)
            np.testing.assert_allclose(actual['beta'], b[:3], rtol=1e-10, atol=1e-12)
            np.testing.assert_allclose(actual['covariance'], covariance, rtol=1e-9, atol=1e-12)
            np.testing.assert_allclose(actual['rss'], rss, rtol=1e-12)
            assert actual['rank'] == rank and actual['n'] == m and actual['correction_df'] == correction_df
            records.append(dict(disconnected=disconnected, kind=kind, reversed=reverse, vce=vce, status='pass',
                covariance_max_error=float(np.max(np.abs(actual['covariance'] - covariance)))))
    (root / 'results.json').write_text(json.dumps(records, indent=2) + '\n')
    print('SCHUR_INFERENCE_VS_FULL_DUMMY_SVD_PASS', len(records), flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('root', type=Path)
    check(p.parse_args().root)
