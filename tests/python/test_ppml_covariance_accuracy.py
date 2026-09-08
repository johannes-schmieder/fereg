from decimal import Decimal
from fractions import Fraction
from pathlib import Path
import sys
import unittest
from unittest import mock

try:
    import numpy as np
    import scipy
except ImportError as error:
    raise unittest.SkipTest('Covariance proof tests require NumPy/SciPy; skip is not admission') from error
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from ppml_covariance_accuracy import covariance_perturbation, fixed_mean_covariance_accuracy
import ppml_covariance_accuracy as accuracy


class CovariancePerturbationTests(unittest.TestCase):
    def test_fixed_mean_bound_against_full_dummy_covariance(self):
        rng = np.random.default_rng(918)
        worker = np.repeat(np.arange(10), 6)
        firm = (worker + np.tile(np.arange(3), 20)) % 10
        groups = np.column_stack((worker, firm))
        x = rng.normal(size=(60, 2))
        y = (1+rng.poisson(2, size=60)).astype(float)
        mu = np.exp(rng.normal(scale=.1, size=60))
        result = fixed_mean_covariance_accuracy(y, x, mu, groups, firm_multiplier=1)
        d = np.column_stack([worker == j for j in range(10)] + [firm == j for j in range(1, 10)])
        design = np.column_stack((x, d))
        bread = np.linalg.inv(design.T @ (mu[:, None]*design))
        scores = design*(y-mu)[:, None]
        v = 60/59 * bread @ (scores.T@scores) @ bread
        for i in range(2):
            for j in range(2):
                lo, hi = map(float, result['covariance_intervals'][i][j])
                self.assertLessEqual(lo, v[i,j])
                self.assertGreaterEqual(hi, v[i,j])
        self.assertFalse(result['state_accuracy_certified'])
        bounded = fixed_mean_covariance_accuracy(y, x, mu, groups, firm_multiplier=1,
                                                 optimum_log_error_upper='0.001')
        changed = mu*np.exp(.001*np.sin(np.arange(60)))
        changed_bread = np.linalg.inv(design.T @ (changed[:, None]*design))
        changed_scores = design*(y-changed)[:, None]
        changed_v = 60/59 * changed_bread @ (changed_scores.T@changed_scores) @ changed_bread
        for i in range(2):
            for j in range(2):
                lo, hi = map(float, bounded['covariance_intervals'][i][j])
                self.assertLessEqual(lo, changed_v[i,j])
                self.assertGreaterEqual(hi, changed_v[i,j])
        self.assertFalse(bounded['fixed_binary_mean_covariance_certified'])
        self.assertTrue(bounded['optimum_covariance_conditional_on_log_bound'])
        with self.assertRaises(MemoryError):
            with mock.patch.object(accuracy, '_ring_project', side_effect=AssertionError('allocated before preflight')):
                fixed_mean_covariance_accuracy(y, x, mu, groups, firm_multiplier=1, memory_bytes=1)
        with self.assertRaises(ValueError):
            fixed_mean_covariance_accuracy(y, x, mu, groups, firm_multiplier=2)

    def test_gram_range_and_symmetric_enclosure(self):
        with self.assertRaisesRegex(ValueError, 'FP64'):
            accuracy._gram_enclosure(np.full((8, 2), 1e-120), np.ones(8))
        with self.assertRaisesRegex(ValueError, 'FP64'):
            accuracy._gram_enclosure(np.ones((8, 2)), np.full(8, 1e-120))
        x = np.array([[.1, .2], [.3, -.4], [.5, .6]])
        w = np.array([.7, .8, .9])
        h, bound, _ = accuracy._gram_enclosure(x, w)
        self.assertTrue(np.array_equal(h, h.T))
        row_errors = []
        for i in range(2):
            total = Fraction(0)
            for j in range(2):
                exact = sum(Fraction(x[t,i])*Fraction(w[t])*Fraction(x[t,j]) for t in range(3))
                total += abs(exact-Fraction(h[i,j]))
            row_errors.append(total)
        self.assertLessEqual(max(row_errors), Fraction(bound))

    def test_exact_scalar_rational_oracle(self):
        result = covariance_perturbation([[2.]], [[3.]], [[0.5]],
            hessian_error_upper='0', meat_error_upper='0', observations=11)
        lo, hi = result['covariance_intervals'][0][0]
        exact = Fraction(11, 10)*Fraction(3, 4)
        self.assertLessEqual(Fraction(Decimal(lo)), exact)
        self.assertGreaterEqual(Fraction(Decimal(hi)), exact)
        self.assertFalse(result['campaign_admission'])
        self.assertFalse(result['state_accuracy_certified'])
        self.assertEqual(result, covariance_perturbation([[2.]], [[3.]], [[0.5]],
            hessian_error_upper='0', meat_error_upper='0', observations=11))

    def test_scalar_projection_and_inexact_inverse_errors(self):
        result = covariance_perturbation([[2.]], [[3.]], [[0.49]],
            hessian_error_upper='0.1', meat_error_upper='0.2', observations=11)
        lo, hi = map(Decimal, result['covariance_intervals'][0][0])
        for h in (Fraction(19, 10), Fraction(21, 10)):
            for m in (Fraction(14, 5), Fraction(16, 5)):
                exact = Fraction(11, 10)*m/(h*h)
                self.assertLessEqual(Fraction(lo), exact)
                self.assertGreaterEqual(Fraction(hi), exact)

    def test_multivariate_perturbation_encloses_dense_oracle(self):
        h = np.array([[3., .2], [.2, 2.]])
        m = np.array([[2., -.1], [-.1, 1.]])
        b = np.linalg.inv(h) + np.array([[.001, .002], [-.001, .001]])
        result = covariance_perturbation(h, m, b, hessian_error_upper='.01',
                                         meat_error_upper='.02', observations=60)
        for sign in (-1, 1):
            true_h = h + sign*np.diag([.005, -.006])
            true_m = m + sign*np.array([[.01, .002], [.002, -.01]])
            inv = np.linalg.inv(true_h)
            v = 60/59 * inv @ true_m @ inv
            for i in range(2):
                for j in range(2):
                    lo, hi = map(float, result['covariance_intervals'][i][j])
                    self.assertLessEqual(lo, v[i,j])
                    self.assertGreaterEqual(hi, v[i,j])

    def test_closed_domain(self):
        for error in ('-1', 'NaN', 'Infinity', '2'):
            with self.assertRaises(ValueError):
                covariance_perturbation([[2.]], [[3.]], [[.5]],
                    hessian_error_upper=error, meat_error_upper='0', observations=10)
        with self.assertRaises(ValueError):
            covariance_perturbation([[2., 1.], [0., 2.]], np.eye(2), np.eye(2),
                hessian_error_upper='0', meat_error_upper='0', observations=10)


if __name__ == '__main__':
    unittest.main()
