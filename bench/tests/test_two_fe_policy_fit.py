"""Small independent rational witnesses; no measurements or calibrated constants."""
from dataclasses import replace
from fractions import Fraction as F
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import two_fe_policy_fit as policy


class EncodedArithmetic(unittest.TestCase):
    def setUp(self):
        q = policy.SCALE
        self.c = policy.Coefficients(q // 2, 3 * q // 2, q // 4, q // 2,
                                     q // 2, q // 8, q // 4, q // 8, q // 4)
        self.structure = policy.Structure(2, 1, 1, 2)
        self.execution = policy.Execution(2, 3, 2)
        self.work = policy.Work(4, 5)

    def price(self, future=(), **changes):
        return policy.evaluate(self.c, self.structure, replace(self.execution, **changes),
                               self.work, future)

    def test_hand_rational_table_and_actual_workers(self):
        # O=68,V=288,Z=9,S=45,P=6. J=144+floor(102/2)=195.
        # B=ceil(45/4)+ceil(ceil(6/2)/2)=14; C=ceil(205/4)+ceil(805/8)=153.
        self.assertEqual(self.price(), policy.Prices(780, 14, 5, 45, 10, 153,
                                                    0, 0, 0, 627, True))
        single = self.price(workers=1)
        self.assertEqual((single.incumbent, single.setup, single.application), (984, 15, 5))
        self.assertNotEqual(self.price(stride=2).incumbent, self.price().incumbent)
        self.assertEqual(self.price(stride=2).startup, 10)

    def test_original_width_and_strict_equality(self):
        q = policy.SCALE
        c = policy.Coefficients(5*q, 0, 0, 563*q, q, 0, 0, 0, 0)
        args = (c, policy.Structure(1, 1, 1, 1), policy.Execution(2, 2, 1), policy.Work(1, 1))
        p = policy.evaluate(*args)
        self.assertEqual((p.incumbent, p.setup, p.startup), (1420, 1126, 10))
        self.assertFalse(p.build)  # 4*1420 == 5*(1126+10); a=1 does not replace r=2.
        self.assertTrue(policy.strict_build(1420, 1126 + 5))

    def test_future_losses_multiplicity_and_reserve(self):
        p = self.price((policy.FutureGroup(3, 5, 50, None),))
        self.assertEqual((p.future_actions, p.future_reserve, p.future_gain, p.net_gain),
                         (50, 63, -813, -186))
        self.assertFalse(p.build)
        p = self.price((policy.FutureGroup(2, 3, 2, self.work),
                        policy.FutureGroup(1, 3, 1, None)))
        # Each full block: floor((780-10-153)/2)=308, then multiplicity two.
        self.assertEqual((p.future_actions, p.future_reserve, p.future_gain), (3, 4, 607))

    def test_compatible_zero_history_and_grouped_schedule(self):
        self.assertEqual(policy.history_work(3, 2, 4, 8, 6), policy.Work(2, 4))
        self.assertEqual(policy.history_work(3, 2, 5, 9, 6), policy.Work(3, 5))
        zero = policy.history_work(3, 2, 0, 0, 6)
        self.assertEqual(zero, policy.Work(0, 0))
        groups = policy.grouped_future(23, 10, 12, True,
                                       history_width=10, history=policy.Work(4, 20))
        self.assertEqual(groups, (policy.FutureGroup(10, 12, 2, policy.Work(4, 20)),
                                  policy.FutureGroup(3, 12, 1, None),
                                  policy.FutureGroup(1, 12, 1, None)))
        self.assertEqual(policy.grouped_future(0, 1, 3, True),
                         (policy.FutureGroup(1, 3, 1, None),))
        self.assertEqual(policy.grouped_future(0, 1, 3, False), ())
        self.assertEqual(policy.grouped_future(0, 10, 1, True),
                         (policy.FutureGroup(1, 1, 1, None),))
        p = self.price((policy.FutureGroup(2, 3, 1, policy.Work(0, 0)),))
        self.assertEqual(p.future_gain, -64)  # Startup10 + C52 + reserve2.

    def test_invalid_raw_history_cannot_hide_behind_rounded_averages(self):
        for values in [(3, 2, 4, 8, 5), (3, 2, 4, 3, 6), (3, 2, 4, 13, 6),
                       (3, 0, 0, 0, 0), (0, 2, 0, 0, 0), (3, 2, 0, 1, 6)]:
            with self.subTest(values=values), self.assertRaises(policy.Unknown):
                policy.history_work(*values)
        with self.assertRaises(policy.Unknown):
            policy.history_work(2, policy.U128_MAX, 0, 0, policy.U128_MAX)
        with self.assertRaises(policy.Unknown):
            policy.grouped_future(3, 2, 1, True)
        with self.assertRaises(policy.Unknown):
            policy.grouped_future(3, 2, 2, True, history_width=2)

    def test_checked_rounding_without_bias_overflow(self):
        self.assertEqual(policy.up(1, policy.U128_MAX), 1 << 96)
        self.assertEqual(policy.ceil_div(policy.U128_MAX, 2), 1 << 127)
        self.assertEqual((policy.down(1, 1), policy.up(1, 1)), (0, 1))
        with self.assertRaises(policy.Unknown):
            policy.up(2, policy.U128_MAX)
        with self.assertRaises(policy.Unknown):
            policy.strict_build(policy.I128_MAX // 4 + 1, 0)
        with self.assertRaises(policy.Unknown):
            policy.strict_build(0, policy.I128_MAX // 5 + 1)
        with self.assertRaises(policy.Unknown):
            policy.evaluate(self.c, self.structure, self.execution,
                            policy.Work(policy.U128_MAX, policy.U128_MAX))

    def test_unknowns_do_not_manufacture_credit(self):
        for changes in ({"workers": 0}, {"stride": 1}, {"r": True}, {"workers": 1 << 64}):
            with self.subTest(changes=changes), self.assertRaises(policy.Unknown):
                self.price(**changes)
        with self.assertRaises(policy.Unknown):
            policy.evaluate(replace(self.c, a_0=0), self.structure, self.execution, self.work,
                            (policy.FutureGroup(1, 3, 1, None),))
        with self.assertRaises(policy.Unknown):
            self.price((policy.FutureGroup(1, 3, 1, None),) * 4)
        with self.assertRaises(policy.Unknown):
            policy.Coefficients(-1, 0, 0, 0, 0, 0, 0, 0, 0)


class ExactEnvelope(unittest.TestCase):
    def fit(self, features, labels, upper, cells=None):
        cells = range(len(labels)) if cells is None else cells
        return policy.fit_envelope([policy.FitPoint(tuple(x), y, cell)
                                    for x, y, cell in zip(features, labels, cells)], upper=upper)

    def test_one_coefficient_and_zero_rows(self):
        self.assertEqual(self.fit([(2,), (5,), (0,)], [10, 20, 0], True), (F(11, 2),))
        self.assertEqual(self.fit([(2,), (5,), (0,)], [10, 20, 8], False), (F(18, 5),))
        with self.assertRaisesRegex(policy.Unsupported, "infeasible"):
            self.fit([(1,), (0,)], [10, 1], True)

    def test_pair_intersection_and_lower_maximization(self):
        self.assertEqual(self.fit([(1, 2), (2, 1)], [10, 10], True), (F(11, 3), F(11, 3)))
        self.assertEqual(self.fit([(1, 0), (0, 1), (1, 1), (2, 0)],
                                 [10, 20, 20, 20], False), (F(9), F(9)))

    def test_equal_cell_weight_and_lexicographic_axis_tie(self):
        # Ten labels in A still get one cell's weight. Both axes have equal
        # objective; lexicographic order selects (0,11). Raw row weights would
        # instead select (11,0), making this a discriminating weighting witness.
        self.assertEqual(self.fit([(0, 1)] * 10 + [(1, 0), (1, 1)],
                                 [0] * 11 + [10], True,
                                 ["A"] * 10 + ["B", "C"]), (F(0), F(11)))
        self.assertEqual(self.fit([(1, 0), (1, 0), (0, 1)], [1, 100, 1], True,
                                 ["A", "A", "B"]), (F(110), F(11, 10)))

    def test_global_rank_missing_nonfinite_and_negative_rejection(self):
        for features, labels in [([(0,)], [0]), ([(1, 2), (2, 4)], [1, 2])]:
            with self.assertRaisesRegex(policy.Unsupported, "rank"):
                self.fit(features, labels, True)
        for label in [None, float("nan"), float("inf"), 1.0, True, -1]:
            with self.subTest(label=label), self.assertRaises(policy.Unsupported):
                self.fit([(1,)], [label], True)
        with self.assertRaises(policy.Unsupported):
            self.fit([(-1,)], [1], True)
        with self.assertRaises(policy.Unsupported):
            policy.fit_envelope([], upper=True)

    def test_quantize_once_direction_and_range(self):
        self.assertEqual(policy.quantize(F(1, 3), upper=False), 1431655765)
        self.assertEqual(policy.quantize(F(1, 3), upper=True), 1431655766)
        self.assertEqual(policy.paired_mean(3, 4), F(7, 2))
        self.assertEqual(policy.paired_mean(policy.U128_MAX, policy.U128_MAX), policy.U128_MAX)
        with self.assertRaises(policy.Unsupported):
            policy.quantize(F(1 << 32), upper=True)
        with self.assertRaises(policy.Unknown):
            policy.paired_mean(1.5, 2)


def synthetic_rows():
    # Hand table: O68,V288,Z9,S45,P6. I rows fit i_s=i_p=9;
    # B/A/R fit 11. Encoded J is 3204/2898/2745; A99,startup198.
    # C features: (3402,6705),(3096,12087),(2943,22851), fitting t0=t1=11.
    # Raw future loss is -99; observed -200 requires reserve >=121, kappa=121/99.
    return [policy.LabelRow(("input", "Fast", workers, "synthetic"),
                            policy.Structure(2, 1, 1, 2), policy.Execution(2, 3, workers),
                            policy.Work(h, d), (policy.FutureGroup(1, 3, 1, None),),
                            incumbent, setup, 180, 3560, continuation, -200)
            for workers, h, d, incumbent, setup, continuation in [
                (1, 2, 3, 7120, 510, 101070),
                (2, 4, 5, 12880, 480, 151830),
                (4, 8, 9, 24400, 465, 257940)]]


class StagedFit(unittest.TestCase):
    def test_exact_stages_and_encoded_suffix_envelope(self):
        result = policy.fit(synthetic_rows())
        q = policy.SCALE
        self.assertEqual(result.coefficients, policy.Coefficients(
            9*q, 9*q, 11*q, 11*q, 11*q, 11*q, 11*q, 11*q,
            (121*q + 98) // 99))
        self.assertEqual(result.rational["kappa_f"], F(121, 99))
        self.assertEqual((result.rows, result.cells, result.suffix_rows), (3, 3, 3))
        for row in synthetic_rows():
            price = policy.evaluate(result.coefficients, row.structure, row.execution, row.work, row.future)
            self.assertLessEqual(price.incumbent, F(9, 10) * row.incumbent_ns)
            self.assertGreaterEqual(price.startup, F(11, 10) * row.startup_ns)
            self.assertLessEqual(price.future_gain, -220)

    def test_no_future_needs_no_reserve_but_does_not_grant_suffix_support(self):
        rows = [replace(row, future=(), observed_suffix_gain=None) for row in synthetic_rows()]
        result = policy.fit(rows)
        self.assertEqual(result.coefficients.kappa_f, 0)
        self.assertEqual(result.suffix_rows, 0)

    def test_absent_labels_fail_instead_of_dropping_rows(self):
        for field in ("incumbent_ns", "setup_ns", "startup_ns", "restart_ns",
                      "continuation_ns", "observed_suffix_gain"):
            rows = synthetic_rows()
            rows[1] = replace(rows[1], **{field: None})
            with self.subTest(field=field), self.assertRaises(policy.Unsupported):
                policy.fit(rows)
        with self.assertRaises(policy.Unsupported):
            policy.fit([])

    def test_rank_and_zero_application_block_required_future(self):
        rows = [replace(row, work=policy.Work(1, 2)) for row in synthetic_rows()]
        with self.assertRaisesRegex(policy.Unsupported, "rank"):
            policy.fit(rows)  # For r2, H1,D2, the two C columns coincide.
        rows = [replace(row, startup_ns=0) for row in synthetic_rows()]
        with self.assertRaisesRegex(policy.Unsupported, "unpriced future"):
            policy.fit(rows)

    def test_encoded_inequalities_are_checked_after_quantization(self):
        # An erroneous all-zero encoder still satisfies lower I, but must fail B.
        with patch.object(policy, "quantize", return_value=0):
            with self.assertRaisesRegex(policy.Unsupported, "encoded setup_ns"):
                policy.fit(synthetic_rows())


if __name__ == "__main__":
    unittest.main()
