"""Tamper tests for the independent, fixed-beta public-state audit."""

import importlib.util
import json
import math
from pathlib import Path
import unittest
from unittest import mock
import tempfile
import sys
from ppml_explicit_dummy_oracle import fit_explicit_ppml

try:
    import numpy as np
    import scipy
except ImportError as error:
    raise unittest.SkipTest(
        "Independent auto science audit requires NumPy and SciPy; "
        "a skipped test is not auto-admission evidence") from error

SPEC = importlib.util.spec_from_file_location(
    "ppml_auto_science", Path(__file__).resolve().parents[2]
    / "scripts/ppml_auto_science.py")
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))


class CoefficientBindingTests(unittest.TestCase):
    def setUp(self):
        rng = np.random.default_rng(29)
        self.x = rng.normal(size=(240, 3))
        self.beta = np.array([0.1, -0.2, 0.3])
        row = np.arange(240)
        self.groups = np.column_stack((row % 8, (row // 8) % 6, row % 5))
        self.eta = self.x @ self.beta + sum(
            0.03 * self.groups[:, j] for j in range(3))
        self.mu = np.exp(self.eta)

    def test_correct_fixed_beta_and_fe_permutations(self):
        for order in ([0, 1, 2], [2, 0, 1], [1, 2, 0]):
            result = AUDIT.coefficient_binding(
                self.x, self.beta, self.mu, self.groups[:, order])
            self.assertTrue(result['coefficient_binding_pass'], result)

    def test_wrong_beta_rejected_even_when_predictor_is_feasible(self):
        wrong = self.beta.copy()
        wrong[0] += 0.01
        result = AUDIT.coefficient_binding(self.x, wrong, self.mu, self.groups)
        self.assertTrue(result['recurrence_converged'])
        self.assertFalse(result['coefficient_binding_pass'])
        self.assertGreater(result['fresh_residual_relative'], 1e-4)

    def test_predictor_outside_model_rejected(self):
        mu = self.mu.copy()
        mu[0] *= 1.01
        self.assertFalse(AUDIT.coefficient_binding(
            self.x, self.beta, mu, self.groups)['coefficient_binding_pass'])

    def test_k_zero_and_disconnected_redundant_effects(self):
        groups = np.column_stack((np.arange(240) % 8,) * 2)
        result = AUDIT.coefficient_binding(
            np.empty((240, 0)), np.empty(0), np.exp(groups[:, 0] * 0.03), groups)
        self.assertTrue(result['coefficient_binding_pass'], result)

    def test_memory_and_iteration_budget_fail_closed(self):
        with self.assertRaises(MemoryError):
            AUDIT.coefficient_binding(self.x, self.beta, self.mu, self.groups,
                                      memory_bytes=1)
        result = AUDIT.coefficient_binding(
            self.x, self.beta, self.mu, self.groups, max_iterations=1)
        self.assertFalse(result['recurrence_converged'])
        self.assertFalse(result['coefficient_binding_pass'])

    def test_nonfinite_and_invalid_domains_rejected(self):
        for value in (0.0, -1.0, np.nan, np.inf):
            mu = self.mu.copy()
            mu[0] = value
            with self.assertRaises(ValueError):
                AUDIT.coefficient_binding(self.x, self.beta, mu, self.groups)

    def test_reporting_deterministic(self):
        args = (self.x, self.beta, self.mu, self.groups)
        self.assertEqual(AUDIT.coefficient_binding(*args),
                         AUDIT.coefficient_binding(*args))


class ObjectiveScoreTests(unittest.TestCase):
    def test_saturated_state_and_poisson_identity(self):
        y = np.array([1.0, 2.0, 3.0, 4.0])
        groups = np.array([[0, 0], [0, 1], [1, 0], [1, 1]])
        result = AUDIT.objective_score(y, np.empty((4, 0)), y, groups)
        self.assertTrue(result['stationarity_pass'])
        self.assertEqual(result['deviance'], 0.0)
        saturated = sum(v * math.log(v) - v - math.lgamma(v + 1) for v in y)
        self.assertAlmostEqual(result['log_likelihood'], saturated)
        shifted = AUDIT.objective_score(y, np.zeros((4, 1)), y * 1.2, groups)
        self.assertFalse(shifted['stationarity_pass'])
        self.assertAlmostEqual(shifted['deviance'],
                               2 * (saturated - shifted['log_likelihood']))

    def test_every_fe_block_is_checked(self):
        y = np.ones(8) * 2
        mu = np.array([1., 3., 1., 3., 1., 3., 1., 3.])
        groups = np.column_stack((np.zeros(8), np.arange(8) % 2))
        result = AUDIT.objective_score(y, np.empty((8, 0)), mu, groups)
        self.assertEqual(result['finite_score_max'], 0.0)
        self.assertEqual(result['fe_score_maxima'][0], 0.0)
        self.assertGreater(result['fe_score_maxima'][1], 1e-8)
        self.assertFalse(result['stationarity_pass'])

    def test_nonpositive_y_not_silently_admitted(self):
        with self.assertRaises(ValueError):
            AUDIT.objective_score([0, 1], np.empty((2, 0)), [1, 1], [[0], [0]])

    def test_overflow_cannot_zero_a_score(self):
        with np.errstate(over='ignore'), self.assertRaises(ValueError):
            AUDIT.objective_score([2, 1], [[1e160], [-1e160]], [1.5, 1.5], [[0], [0]])

    def test_underflow_cannot_zero_a_score(self):
        with self.assertRaises(ValueError):
            AUDIT.objective_score([2, 1], [[1e-200], [-1e-200]], [1.5, 1.5], [[0], [0]])


class CovarianceTests(unittest.TestCase):
    def test_direct_ring_audit_matches_dense_oracle(self):
        rng = np.random.default_rng(391)
        worker = np.repeat(np.arange(10), 6)
        firm = (worker + np.tile(np.arange(3), 20)) % 10
        groups = np.column_stack((worker, firm))
        x = rng.normal(size=(60, 2))
        y = (1+rng.poisson(2, size=60)).astype(float)
        dummy = np.column_stack([worker == j for j in range(10)] + [firm == j for j in range(10)])
        oracle = fit_explicit_ppml(np.column_stack((x,dummy)).tolist(), y.tolist(),
                                   vce='robust', covariance_correction=60/59)
        result = AUDIT.control_covariance(y,x,oracle.mean,groups,ring_multiplier=1)
        np.testing.assert_allclose(result['control_covariance'],
                                   np.asarray(oracle.covariance)[:2,:2],rtol=1e-10,atol=1e-12)
        self.assertTrue(all(p['method']=='direct_ring' and p['stop'] is None for p in result['projections']))
        binding = AUDIT.coefficient_binding(x,oracle.coefficients[:2],oracle.mean,groups,ring_multiplier=1)
        self.assertTrue(binding['coefficient_binding_pass'])
        self.assertIsNone(binding['recurrence_converged'])
        wrong = np.asarray(oracle.coefficients[:2])+0.01
        self.assertFalse(AUDIT.coefficient_binding(x,wrong,oracle.mean,groups,ring_multiplier=1)['coefficient_binding_pass'])
        with self.assertRaises(ValueError):
            AUDIT.control_covariance(y,x,oracle.mean,groups,ring_multiplier=2)

    def test_full_dummy_oracle_one_two_three_effects(self):
        rng = np.random.default_rng(293)
        n = 120
        x = rng.normal(size=(n, 2))
        row = np.arange(n)
        all_groups = np.column_stack((row % 3, (row // 3) % 4, (row // 12) % 2))
        y = (1 + rng.poisson(np.exp(0.1 * x[:, 0]))).astype(float)
        for effects in (1, 2, 3):
            groups = all_groups[:, :effects]
            dummy = np.column_stack([
                (groups[:, j] == level).astype(float)
                for j in range(effects) for level in np.unique(groups[:, j])])
            design = np.column_stack((x, dummy))
            oracle = fit_explicit_ppml(design.tolist(), y.tolist(), vce='robust',
                                       covariance_correction=n / (n - 1))
            result = AUDIT.control_covariance(y, x, oracle.mean, groups)
            np.testing.assert_allclose(result['control_covariance'],
                                       np.asarray(oracle.covariance)[:2, :2],
                                       rtol=1e-10, atol=1e-12)
            self.assertEqual(result['finite_rank'], 2)
            self.assertTrue(AUDIT.coefficient_binding(
                x, oracle.coefficients[:2], oracle.mean, groups)['coefficient_binding_pass'])
            self.assertTrue(AUDIT.objective_score(
                y, x, oracle.mean, groups)['stationarity_pass'])

    def test_rank_memory_and_iteration_failure_closed(self):
        rng = np.random.default_rng(4)
        x = rng.normal(size=(60, 2))
        groups = rng.integers(0, 5, size=(60, 2))
        y = np.ones(60) * 2
        mu = np.linspace(1, 3, 60)
        with self.assertRaises(MemoryError):
            AUDIT.control_covariance(y, x, mu, groups, memory_bytes=1)
        with self.assertRaises(ValueError):
            AUDIT.control_covariance(y, x, mu, groups, max_iterations=1)
        with self.assertRaises(ValueError):
            AUDIT.control_covariance(y, np.column_stack((x[:, 0], x[:, 0])), mu, groups)
        for column in (np.ones(60), (groups[:, 0] == 0).astype(float)):
            with self.assertRaises(ValueError):
                AUDIT.control_covariance(y, column[:, None], mu, groups)

    def test_k_zero(self):
        result = AUDIT.control_covariance([1, 2, 3], np.empty((3, 0)),
                                          [2, 2, 2], [[0], [0], [0]])
        self.assertEqual(result['finite_rank'], 0)
        self.assertEqual(result['control_covariance'], [])


class DofTests(unittest.TestCase):
    def test_connected_cartesian(self):
        row = np.arange(24)
        groups = np.column_stack((row % 3, (row // 3) % 4, row // 12))
        for j, expected in ((1, 2), (2, 5), (3, 6)):
            result = AUDIT.categorical_dof(groups[:, :j])
            self.assertEqual(result['df_a'], expected)
            self.assertEqual(result['pairwise_approximation'], j == 3)

    def test_repeated_partition_and_disconnected_pairs(self):
        groups = np.column_stack((np.arange(12) % 3,) * 3)
        result = AUDIT.categorical_dof(groups)
        self.assertEqual(result['redundant'], [1, 3, 3])
        self.assertEqual(result['df_a'], 2)


class AdmissionInventoryTests(unittest.TestCase):
    def test_reference_interval_comparison_retains_serialization_uncertainty(self):
        result = AUDIT.compare_reference_coefficients([0.004], ['4.00000150000000e-03'], '4e-12')
        self.assertTrue(result[0]['outside_certified_optimum_envelope'])
        self.assertFalse(AUDIT.compare_reference_coefficients([0.004], ['4.00000000000000e-03'], '4e-12')[0]['outside_certified_optimum_envelope'])
        self.assertFalse(AUDIT.compare_reference_coefficients([0.004], ['0.004'], '4e-12')[0]['outside_certified_optimum_envelope'])
        for radius in ('NaN', '-1', '1e-8'):
            with self.assertRaises(ValueError):
                AUDIT.compare_reference_coefficients([0.004], ['0.004'], radius)



class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.prefix = Path(self.directory.name) / 'capture'
        self.y = np.ones(12) * 2
        self.x = np.empty((12, 0))
        self.groups = (np.arange(12) % 3)[:, None]
        score = AUDIT.objective_score(self.y, self.x, self.y, self.groups)
        scalars = {'N': 12, 'll': score['log_likelihood'], 'deviance': 0,
                   'outer_tolerance': 1e-8,
                   'rank': 1, 'df_a': 2, 'df_resid': 9, 'converged': 1, 'feppml_threads': 1,
                   'feppml_solver_fallback_count': 0}
        solver = np.zeros((4, 22))
        solver[:, :6] = [1, 2, 1, 2, 1, 2]
        solver[:, 6] = np.arange(1, 5)
        certificate = np.zeros((16, 1))
        certificate[-1, 0] = 1
        matrices = {'b': np.zeros((1, 1)), 'V': np.zeros((1, 1)),
                    'feppml_certificate': certificate, 'feppml_solver': solver}
        columns = {'b': ['o._cons'], 'V': ['o._cons'],
                   'feppml_certificate': ['value'], 'feppml_solver': AUDIT.SOLVER_COLUMNS}
        lines = [f'scalar\t{name}\t{value}' for name, value in scalars.items()]
        for name, matrix in matrices.items():
            lines.append(f'columns\t{name}\t' + ' '.join(columns[name]))
            for i, row in enumerate(matrix):
                for j, value in enumerate(row):
                    lines.append(f'matrix\t{name}\t{i+1}\t{j+1}\t{value}')
        self.tsv = Path(str(self.prefix) + '.tsv')
        self.tsv.write_text('\n'.join(lines) + '\n')
        self.csv = Path(str(self.prefix) + '.csv')
        self.csv.write_text('audit_row,audit_sample,y,g1,audit_mu\n' + ''.join(
            f'{i+1},1,2,{i%3},2\n' for i in range(12)))

    def audit(self):
        return AUDIT.audit_capture(self.prefix, y=self.y, x=self.x,
                                   groups=self.groups, threads=1)

    def test_complete_capture(self):
        self.assertTrue(self.audit()['independent_candidate_pass'])

    def test_binding_failure_reports_diagnostics_before_covariance_work(self):
        with mock.patch.object(AUDIT, 'coefficient_binding', return_value={'coefficient_binding_pass': False, 'iterations': 123}), \
             mock.patch.object(AUDIT, 'control_covariance', side_effect=AssertionError('unnecessary covariance work')):
            with self.assertRaisesRegex(ValueError, '"iterations": 123'):
                self.audit()

    def test_stability_requires_two_independently_valid_states(self):
        tighter = Path(self.directory.name) / 'tighter'
        Path(str(tighter)+'.csv').write_text(self.csv.read_text())
        tight_tsv = Path(str(tighter)+'.tsv')
        tight_tsv.write_text(self.tsv.read_text().replace('1e-08', '1e-10'))
        args = dict(y=self.y, x=self.x, groups=self.groups, threads=1)
        self.assertTrue(AUDIT.audit_stability(self.prefix, tighter, **args)['outer_score_stability_pass'])
        tight_tsv.write_text(self.tsv.read_text())
        with self.assertRaises(ValueError):
            AUDIT.audit_stability(self.prefix, tighter, **args)
        tight_tsv.write_text(self.tsv.read_text().replace('1e-08', '1e-10'))
        Path(str(tighter)+'.csv').write_text(self.csv.read_text().replace('1,1,2,0,2', '1,1,2,0,3'))
        with self.assertRaises(ValueError):
            AUDIT.audit_stability(self.prefix, tighter, **args)

    def test_missing_duplicate_and_stripe_tampering(self):
        original = self.tsv.read_text()
        for text in (original + 'scalar\tN\t12\n',
                     original.replace('scalar\tN\t12\n', ''),
                     original.replace('columns\tb\to._cons', 'columns\tb\t_cons'),
                     original.replace('matrix\tV\t1\t1\t0.0\n', '')):
            self.tsv.write_text(text)
            with self.assertRaises(ValueError):
                self.audit()

    def test_input_sample_and_result_tampering(self):
        original = self.csv.read_text()
        for text in (original.replace('1,1,2,0,2', '1,0,2,0,2'),
                     original.replace('1,1,2,0,2', '2,1,2,0,2'),
                     original.replace('1,1,2,0,2', '1,1,3,0,2'),
                     original.replace('1,1,2,0,2', '1,1,2,0,3')):
            self.csv.write_text(text)
            with self.assertRaises(ValueError):
                self.audit()

    def test_rank_route_and_certificate_tampering(self):
        original = self.tsv.read_text()
        for old, new in [('scalar\trank\t1', 'scalar\trank\t2'),
                         ('scalar\tdf_resid\t9', 'scalar\tdf_resid\t8'),
                         ('matrix\tfeppml_solver\t1\t2\t2.0', 'matrix\tfeppml_solver\t1\t2\t4.0'),
                         ('matrix\tfeppml_certificate\t16\t1\t1.0', 'matrix\tfeppml_certificate\t16\t1\t0.0')]:
            self.tsv.write_text(original.replace(old, new))
            with self.assertRaises(ValueError):
                self.audit()


if __name__ == '__main__':
    unittest.main()
