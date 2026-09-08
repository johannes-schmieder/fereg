from fractions import Fraction
import random
import unittest
from unittest import mock
try:
    import numpy as np
except ImportError as error:
    raise unittest.SkipTest('Coefficient-accuracy tests require NumPy') from error
import sys
from pathlib import Path
try:
    import scipy
except ImportError as error:
    raise unittest.SkipTest("Coefficient-accuracy tests require SciPy; skipping is not admission evidence") from error
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from ppml_coefficient_accuracy import point, add, sub, mul, exp, tree, energy_bound, coefficient_accuracy
import ppml_coefficient_accuracy as accuracy
from ppml_explicit_dummy_oracle import fit_explicit_ppml


class IntervalTests(unittest.TestCase):
    def test_arithmetic_encloses_exact_binary_rationals(self):
        rng = random.Random(39)
        for _ in range(1000):
            x = rng.uniform(-1e5, 1e5); y = rng.uniform(-1e5, 1e5)
            for operation, exact in ((add, Fraction(x)+Fraction(y)),
                                     (sub, Fraction(x)-Fraction(y)),
                                     (mul, Fraction(x)*Fraction(y))):
                lo, hi = operation(point(x), point(y))
                self.assertLessEqual(Fraction(lo), exact)
                self.assertGreaterEqual(Fraction(hi), exact)

    def test_tree_bound_encloses_exact_rational_flow_energy(self):
        groups = np.array([[0,0],[0,1],[1,0],[1,1]])
        loads = [point(v) for v in (0.1,-0.2,0.3,-0.4)]
        weights = [point(v) for v in (1.,2.,3.,4.)]
        t = tree(groups)
        worker, firm, parent, edge, order = t
        exact = [Fraction(0)]*len(parent)
        for u,v,value in zip(worker,firm,loads):
            exact[u] += Fraction(value[0]); exact[v] -= Fraction(value[0])
        energy = Fraction(0)
        for v in reversed(order[1:]):
            energy += exact[v]**2 / Fraction(weights[edge[v]][0])
            exact[parent[v]] += exact[v]
        self.assertEqual(exact[0], 0)
        self.assertGreaterEqual(Fraction(energy_bound(t, loads, weights)), energy)

    def test_closed_domains_and_exp_zero(self):
        with self.assertRaises(ValueError):
            point(float('nan'))
        with self.assertRaises(ValueError):
            tree(np.array([[0,0],[1,1]]))
        self.assertLessEqual(exp(point(0))[0], 1)
        self.assertGreaterEqual(exp(point(0))[1], 1)

    def test_full_coefficient_certificate_matches_dense_oracle(self):
        rng = np.random.default_rng(394)
        worker = np.repeat(np.arange(10), 6)
        firm = (worker + np.tile(np.arange(3), 20)) % 10
        groups = np.column_stack((worker, firm))
        x = rng.normal(size=(60, 2))
        y = (1 + rng.poisson(2, size=60)).astype(float)
        design = np.column_stack((x, np.column_stack(
            [worker == j for j in range(10)] + [firm == j for j in range(10)])))
        oracle = fit_explicit_ppml(design.tolist(), y.tolist())
        result = coefficient_accuracy(y, x, oracle.coefficients[:2], oracle.mean,
                                      groups, firm_multiplier=1)
        self.assertTrue(result['coefficient_accuracy_certified'], result)
        self.assertFalse(result['campaign_admission'])
        self.assertGreaterEqual(accuracy.Decimal(result['public_to_optimum_log_mean_upper']),
                                accuracy.Decimal(result['public_to_feasible_log_mean_upper']))
        self.assertLess(float(result['public_to_optimum_log_mean_upper']), 1e-8)
        bad_beta = np.asarray(oracle.coefficients[:2]) + 0.001
        bad = coefficient_accuracy(y, x, bad_beta, oracle.mean, groups, firm_multiplier=1)
        self.assertFalse(bad['coefficient_accuracy_certified'])
        with self.assertRaises(MemoryError):
            coefficient_accuracy(y, x, oracle.coefficients[:2], oracle.mean,
                                 groups, firm_multiplier=1, memory_bytes=1)
        with self.assertRaises(ValueError):
            coefficient_accuracy(y, x, oracle.coefficients[:2], oracle.mean,
                                 groups, firm_multiplier=2)
        original_project = accuracy._ring_project
        calls = 0
        def underflow_residual(*args):
            nonlocal calls
            values = original_project(*args)
            calls += 1
            if calls == 2:
                return np.full_like(values[0], 1e-120), values[1], values[2]
            return values
        with mock.patch.object(accuracy, '_ring_project', side_effect=underflow_residual):
            with self.assertRaisesRegex(ValueError, 'FP64 Gram'):
                coefficient_accuracy(y, x, oracle.coefficients[:2], oracle.mean,
                                     groups, firm_multiplier=1)


if __name__ == '__main__':
    unittest.main()
