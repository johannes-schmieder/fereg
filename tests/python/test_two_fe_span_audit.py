"""Independent small-design calibration and bounded forest failure tests."""

from decimal import Decimal, localcontext
from pathlib import Path
import sys
import unittest
from unittest import mock

import numpy as np
from scipy.linalg import qr

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from two_fe_span_audit import audit_two_fe_span


def decimal_projection(target, groups):
    """100-digit explicit-dummy normal equations, independent of forest code."""
    design = np.column_stack([groups[:, j] == level for j in range(2)
                              for level in np.unique(groups[:, j])]).astype(int)
    _, factor, pivots = qr(design.astype(float), pivoting=True, mode="economic")
    rank = int(np.count_nonzero(np.abs(np.diag(factor)) > 1e-10))
    design = design[:, pivots[:rank]]
    with localcontext() as context:
        context.prec = 100
        values = [Decimal.from_float(float(value)) for value in target]
        gram = [[Decimal(int(value)) for value in row] for row in design.T @ design]
        rhs = [sum((value for value, selected in zip(values, column) if selected),
                   Decimal(0)) for column in design.T]
        for i in range(rank):
            pivot = gram[i][i]
            for j in range(i+1, rank):
                ratio = gram[j][i] / pivot
                for k in range(i+1, rank):
                    gram[j][k] -= ratio * gram[i][k]
                rhs[j] -= ratio * rhs[i]
        beta = [Decimal(0)] * rank
        for i in reversed(range(rank)):
            beta[i] = (rhs[i] - sum((gram[i][j]*beta[j]
                                     for j in range(i+1, rank)), Decimal(0))) / gram[i][i]
        return np.array([float(value - sum((b for b, selected in zip(beta, row)
                                            if selected), Decimal(0)))
                         for value, row in zip(values, design)])


class TwoFeSpanTests(unittest.TestCase):
    def cases(self):
        chain = np.array([(j, j) for j in range(12)]
                         + [(j+1, j) for j in range(11)])
        cycle = np.concatenate((chain, [[0, 11]]))
        dense = np.array([(j, k) for j in range(4) for k in range(4)])
        bottleneck = np.concatenate((dense, dense+5, [[3, 5]]))
        return dict(chain=chain, cycle=cycle, bottleneck=bottleneck,
                    disconnected=np.concatenate((cycle, cycle+30)),
                    duplicates=np.repeat(cycle, 3, axis=0),
                    singleton_cascade=np.concatenate((cycle, [[20, 20], [21, 20]])))

    def test_exact_witness_matches_high_precision_oracle(self):
        for name, groups in self.cases().items():
            target = groups[:, 0]/16.0 - groups[:, 1]/32.0 + 0.25
            with self.subTest(name=name), mock.patch(
                    "two_fe_span_audit.lsmr", side_effect=AssertionError("unexpected fallback")):
                result = audit_two_fe_span(target, groups)
                self.assertTrue(result['coefficient_binding_pass'], result)
                self.assertEqual(result['method'], 'two_fe_forest')
                self.assertLess(np.linalg.norm(decimal_projection(target, groups)), 1e-90)

    def test_perturbed_cycles_and_parallel_rows_remain_indeterminate(self):
        for name in ('cycle', 'bottleneck', 'disconnected', 'duplicates'):
            groups = self.cases()[name]
            target = groups[:, 0]/16.0 - groups[:, 1]/32.0
            target[0] += 0.125
            result = audit_two_fe_span(target, groups)
            self.assertFalse(result['coefficient_binding_pass'], (name, result))
            self.assertEqual(result['method'], 'two_fe_grounded_scaled_lsmr')
            oracle = decimal_projection(target, groups)
            self.assertAlmostEqual(result['fresh_residual_relative'],
                                   np.linalg.norm(oracle)/max(1, np.linalg.norm(target)), places=12)
            self.assertLess(result['fresh_normal_max'], 1e-12)

    def test_degree_scaled_fallback_checks_original_normals(self):
        groups = np.array([(0, 0), (0, 1), (1, 0), (1, 1)])
        # A cycle perturbation has tiny L2 error but an original per-level
        # normal above the gate for the forest; the LS witness repairs it.
        target = np.array([0., 1., 1., 2.])
        target[0] += 1e-11
        result = audit_two_fe_span(target, groups)
        self.assertEqual(result['method'], 'two_fe_grounded_scaled_lsmr')
        self.assertTrue(result['coefficient_binding_pass'], result)
        self.assertGreater(result['forest']['fresh_normal_max'], 1e-12)
        self.assertLess(result['fresh_normal_max'], 1e-12)

    def test_disconnected_singletons_use_one_traversal(self):
        from scipy.sparse.csgraph import breadth_first_order
        groups = np.column_stack((np.arange(2000), np.arange(2000)))
        with mock.patch('two_fe_span_audit.breadth_first_order', wraps=breadth_first_order) as traverse:
            result = audit_two_fe_span(np.arange(2000)/32., groups)
        self.assertEqual(traverse.call_count, 1)
        self.assertEqual(result['components'], 2000)
        self.assertTrue(result['coefficient_binding_pass'])

    def test_weight_multiplicity_changes_do_not_change_model_space(self):
        groups = self.cases()['cycle']
        repeats = np.arange(len(groups)) % 11 + 1
        weighted_groups = np.repeat(groups, repeats, axis=0)
        target = weighted_groups[:, 0]/16. - weighted_groups[:, 1]/32.
        self.assertTrue(audit_two_fe_span(target, weighted_groups)['coefficient_binding_pass'])

    def test_budget_before_topology_and_one_byte_boundary(self):
        groups = self.cases()['cycle']
        target = groups[:, 0]/16.
        budget = 1280*len(target)
        with mock.patch('two_fe_span_audit.np.unique', side_effect=AssertionError("allocated")):
            with self.assertRaises(MemoryError):
                audit_two_fe_span(target, groups, memory_bytes=budget-1)
        self.assertTrue(audit_two_fe_span(target, groups, memory_bytes=budget)['coefficient_binding_pass'])

    def test_iteration_exhaustion_not_accepted(self):
        groups = self.cases()['bottleneck']
        target = groups[:, 0]/16. - groups[:, 1]/32.
        target[0] += .125
        result = audit_two_fe_span(target, groups, max_iterations=1)
        self.assertFalse(result['coefficient_binding_pass'])
        self.assertEqual(result['recurrence_stop'], 7)

    def test_permutations_and_nonconsecutive_labels(self):
        groups = self.cases()['disconnected']
        target = groups[:, 0]/16. - groups[:, 1]/32.
        for order in (np.arange(len(target)), np.arange(len(target))[::-1]):
            result = audit_two_fe_span(target[order], groups[order, ::-1]*100003-999)
            self.assertTrue(result['coefficient_binding_pass'], result)

    def test_invalid_inputs_fail(self):
        groups = self.cases()['cycle']
        target = np.zeros(len(groups))
        for value in (np.nan, np.inf):
            bad = target.copy(); bad[0] = value
            with self.assertRaises(ValueError):
                audit_two_fe_span(bad, groups)
        with self.assertRaises(ValueError):
            audit_two_fe_span(target, groups+.1)
        with self.assertRaises(ValueError):
            audit_two_fe_span(target, groups, max_iterations=0)
        with self.assertRaises(ValueError):
            audit_two_fe_span(target.astype(complex)+1j, groups)
        with np.errstate(over='ignore'), self.assertRaises(ValueError):
            audit_two_fe_span(np.full(len(groups), 1e300), groups)


if __name__ == '__main__':
    unittest.main()
