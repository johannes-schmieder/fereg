"""Separate scalar dense Newton/sandwich and known-optimum checks."""
from pathlib import Path
import json
import sys
import unittest
try:
    import numpy as np
    import scipy  # noqa: F401  # two_fe_ppml_oracle imports SciPy at module level
except ImportError as error:  # scientific Python is optional for the gate
    raise unittest.SkipTest(f"requires NumPy/SciPy/pandas: {error}") from error

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tests/python'))
from ppml_explicit_dummy_oracle import fit_explicit_ppml
from two_fe_ppml_oracle import fit, fit_nonnegative_sample, zero_intercept_group_sample


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


def validate_nonnegative():
    rng = np.random.default_rng(2026092812)
    records = []
    for disconnected in (False, True):
        g = np.repeat(np.arange(8), 8)
        h = np.tile(np.arange(4), 16) + (4 * (g >= 4) if disconnected else 0)
        X = np.repeat(rng.normal(size=(len(g), 2)), 2, axis=0)
        g, h = np.repeat(g, 2), np.repeat(h, 2)
        beta = np.array([.2, -.1])
        mu = np.exp(X @ beta + .1 * np.sin(g) - .05 * np.cos(h))
        y = mu * np.tile([0., 2.], len(mu) // 2)
        result = fit_nonnegative_sample(y, X, g, h)
        D = np.column_stack([X, np.eye(g.max() + 1)[g], np.eye(h.max() + 1)[h]])
        independent = fit_explicit_ppml(D.tolist(), y.tolist(), vce='robust',
                                       covariance_correction=len(y) / (len(y) - 1), tolerance=1e-13)
        np.testing.assert_allclose(result['beta'], beta, rtol=1e-11, atol=1e-12)
        np.testing.assert_allclose(result['mu'], mu, rtol=1e-11, atol=1e-12)
        np.testing.assert_allclose(result['beta'], independent.coefficients[:2], rtol=1e-9, atol=1e-11)
        np.testing.assert_allclose(result['covariance'], np.asarray(independent.covariance)[:2, :2], rtol=1e-9, atol=1e-11)
        assert result['rank'] == len(independent.kept_columns)
        records.append(dict(disconnected=disconnected, rank=result['rank'],
                            coefficient_error=float(np.max(abs(result['beta'] - beta))),
                            mean_error=float(np.max(abs(result['mu'] - mu)))))
        try:
            fit(y, X, g, h)
        except ValueError:
            pass
        else:
            raise AssertionError('positive-only entry accepted zero outcomes')
    groups = np.column_stack((np.r_[g, [20] * 4], np.r_[h, [0] * 4]))
    keep, witness = zero_intercept_group_sample(np.r_[y, [0.] * 4], groups)
    assert keep[:-4].all() and not keep[-4:].any() and witness['zero_groups'] == [1, 0]
    order = rng.permutation(len(keep))
    reordered, _ = zero_intercept_group_sample(np.r_[y, [0.] * 4][order], groups[order])
    np.testing.assert_array_equal(reordered, keep[order])
    try:
        zero_intercept_group_sample([1., 0., 1., 1., 1., 1., 1., 1.],
                                    np.column_stack((np.repeat(np.arange(4), 2), [0, 9, 0, 0, 0, 0, 0, 0])))
    except ValueError as error:
        assert 'singleton' in str(error)
    else:
        raise AssertionError('singleton closure was silently claimed')
    return dict(passed=True, nonnegative_known_optima=records, zero_group_witness=True,
                reordered_witness=True, singleton_refusal=True, positive_domain_unchanged=True)


if __name__ == '__main__':
    print(json.dumps(dict(positive=validate(), nonnegative=validate_nonnegative()), indent=2, allow_nan=False))



class GroundedPoissonOracleTests(unittest.TestCase):
    def test_positive_sample(self):
        validate()

    def test_separation_sample(self):
        validate_nonnegative()
