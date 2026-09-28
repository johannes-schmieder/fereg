"""Separate scalar dense Newton/sandwich and known-optimum checks."""
from pathlib import Path
import json
import sys
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tests/python'))
from ppml_explicit_dummy_oracle import fit_explicit_ppml
from two_fe_ppml_oracle import fit


def validate():
    rng = np.random.default_rng(2026092810)
    records = []
    for disconnected in (False, True):
        g = np.repeat(np.arange(8), 12)
        h = np.tile(np.arange(3), 32) + (3 * (g >= 4) if disconnected else 0)
        X = rng.normal(size=(len(g), 3))
        y = 1. + rng.poisson(3, len(g))
        result = fit(y, X, g, h)
        D = np.column_stack([X, np.eye(8)[g], np.eye(h.max() + 1)[h]])
        independent = fit_explicit_ppml(D.tolist(), y.tolist(), vce='robust',
                                       covariance_correction=len(y) / (len(y) - 1), tolerance=1e-13)
        np.testing.assert_allclose(result['beta'], independent.coefficients[:3], rtol=1e-9, atol=1e-11)
        np.testing.assert_allclose(result['mu'], independent.mean, rtol=1e-10, atol=1e-11)
        np.testing.assert_allclose(result['covariance'], np.asarray(independent.covariance)[:3, :3],
                                   rtol=1e-9, atol=1e-11)
        assert result['rank'] == len(independent.kept_columns)
        records.append(dict(disconnected=disconnected, rank=result['rank'],
                            beta_error=float(np.max(abs(result['beta'] - independent.coefficients[:3]))),
                            mean_error=float(np.max(abs(result['mu'] - independent.mean))),
                            covariance_error=float(np.max(abs(result['covariance'] - np.asarray(independent.covariance)[:3, :3])))))
    # Duplicate rows with opposite residuals give a separately known optimum.
    X = np.repeat(X, 2, axis=0)
    g, h = np.repeat(g, 2), np.repeat(h, 2)
    beta = np.array([.2, -.1, .3])
    mu = np.exp(X @ beta + .1 * np.sin(g) - .05 * np.cos(h))
    result = fit(mu + np.tile([.01, -.01], len(mu) // 2), X, g, h)
    np.testing.assert_allclose(result['beta'], beta, rtol=1e-11, atol=1e-12)
    np.testing.assert_allclose(result['mu'], mu, rtol=1e-11, atol=1e-12)
    return dict(passed=True, full_dummy_scalar_oracles=records, nonzero_residual_known_optimum=True)


if __name__ == '__main__':
    print(json.dumps(validate(), indent=2, allow_nan=False))
