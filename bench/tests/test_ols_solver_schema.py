"""Current OLS route coordinates and explicit historical contract boundaries."""
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import native_io
import qualify
import routing_science as science
from test_cmgschur import bits, current_result, result
from test_derived_residual_scope import derived, meta, projected, set_row


class CurrentOlsSolverSchema(unittest.TestCase):
    def test_ordinary_envelope_names_only_current_work_and_magnitudes(self):
        raw = current_result()
        paths = {p for p in science.replay_scope(raw) if '/RSOLVER/' in p}
        self.assertEqual(paths, {f'/matrices/RSOLVER/bits/{20*r+c}'
                               for r in range(2) for c in (10, 14, 15, 16, 17, 18, 19)})
        self.assertEqual(len(native_io.diagnostics(raw, False)['solver_rows'][0]), 22)
        for mutate in (lambda r: r['matrices']['RSOLVER'].update(rows=True),
                       lambda r: r['matrices']['RSOLVER'].update(columns=16)):
            changed = copy.deepcopy(raw)
            mutate(changed)
            with self.assertRaises(ValueError):
                science.replay_scope(changed)

    def test_historical_and_current_contracts_cannot_be_interchanged(self):
        old, current = result(), current_result()
        self.assertEqual(len(native_io.legacy_ols_view(old)['matrices']['RSOLVER']['bits']), 28)
        self.assertEqual(len(native_io.diagnostics(old, False, legacy_ols=True)['solver_rows'][0]), 16)
        for check in (native_io.legacy_ols_view,
                      lambda r: native_io.diagnostics(r, False, legacy_ols=True),
                      lambda r: science.replay_scope(r, historical=True)):
            with self.assertRaises(ValueError):
                check(current)
        for check in (native_io.current_ols_rows, lambda r: native_io.diagnostics(r, False), science.replay_scope):
            with self.assertRaises(ValueError):
                check(old)

    def test_public_tuple_checks_locals_fallback_and_integer_counts(self):
        for column, value in ((0, 3.), (1, 6.), (2, 1.), (3, .5), (4, 1.),
                              (9, 2.**32), (10, .5), (11, 1.), (12, 1.),
                              (13, -1.), (14, float('nan')), (19, float('inf'))):
            changed = current_result()
            set_row(changed, 0, {column: value})
            with self.subTest(column=column), self.assertRaises(ValueError):
                native_io.current_ols_rows(changed)
        empty_block = current_result()
        set_row(empty_block, 0, {3: 2})
        with self.assertRaises(ValueError):
            native_io.current_ols_rows(empty_block)
        set_row(empty_block, 0, {4: 1})
        native_io.current_ols_rows(empty_block)
        set_row(empty_block, 0, {3: 3, 4: 100})
        native_io.current_ols_rows(empty_block)

    def test_request_is_bound_input_not_reduction_code(self):
        raw = current_result()
        for request in ('auto', 'lsmr'):
            native_io.validate_ols_request(raw, request)
        for request in ('map', 'cmg', 'cmgschur'):
            with self.subTest(request=request), self.assertRaises(ValueError):
                native_io.validate_ols_request(raw, request)
        set_row(raw, 0, {0: 2})
        native_io.validate_ols_request(raw, 'auto')
        set_row(raw, 0, {1: 2, 3: 0})
        native_io.validate_ols_request(raw, 'map')

    def test_current_cmg_request_uses_public_tuple_not_historical_solver_id(self):
        raw = current_result()
        set_row(raw, 0, {1: 3, 3: 4})
        native_io.validate_ols_request(raw, 'cmg')
        for fields in ({0: 2}, {1: 4}, {3: 1}, {1: 7}):
            changed = copy.deepcopy(raw)
            set_row(changed, 0, fields)
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                native_io.validate_ols_request(changed, 'cmg')
        set_row(raw, 0, {1: 1, 3: 0})
        native_io.validate_ols_request(raw, 'cmg')
        set_row(raw, 0, {1: 2, 2: 1, 3: 0, 11: 1, 12: 3})
        native_io.validate_ols_request(raw, 'cmg')
        set_row(raw, 0, {11: 0, 12: 0})
        with self.assertRaises(ValueError):
            native_io.validate_ols_request(raw, 'cmg')
        # fe/session/fallback.rs substitutes Auto on unsupported CMG slopes.
        for reduction in (1, 2):
            set_row(raw, 0, {0: reduction, 1: 3, 2: 0, 3: 1, 11: 1, 12: 1})
            native_io.validate_ols_request(raw, 'cmg')

    def test_explicit_lsmr_accepts_only_documented_marked_fallbacks(self):
        # fe/auto.rs's explicit slope tests: nonconvergence -> full PCG;
        # unsupported design -> MAP. Other declines carry the same reason codes.
        for reduction, method, pc in ((1, 1, 0), (1, 2, 0), (1, 3, 1), (1, 3, 2),
                                      (1, 3, 3), (2, 3, 1), (2, 3, 2), (2, 3, 4)):
            for reason in (1, 2, 3, 4):
                raw = current_result()
                fields = {0: reduction, 1: method, 3: pc, 11: 1, 12: reason, 13: 1}
                if pc == 2:
                    fields[4] = 1
                set_row(raw, 0, fields)
                for request in ('lsmr', 'lsmr-reference'):
                    with self.subTest(method=method, pc=pc, reason=reason, request=request):
                        native_io.validate_ols_request(raw, request)
                for mutation in ({11: 0, 12: 0}, {11: 0}, {12: 0}, {12: 5}, {0: 0}, {1: 5}):
                    changed = copy.deepcopy(raw)
                    set_row(changed, 0, mutation)
                    with self.subTest(method=method, mutation=mutation), self.assertRaises(ValueError):
                        native_io.validate_ols_request(changed, 'lsmr')
                raw['matrices']['RCERT']['bits'][-1] = bits(0.)
                with self.assertRaises(ValueError):
                    native_io.validate_ols_request(raw, 'lsmr')
        bad = current_result()
        set_row(bad, 0, {1: 3, 3: 0, 11: 1, 12: 3})
        with self.assertRaises(ValueError):
            native_io.validate_ols_request(bad, 'map')

    def test_explicit_requests_accept_only_the_exact_no_fe_tuple(self):
        raw = current_result()
        for row in range(2):
            set_row(raw, row, {i: 0 for i in range(16)})
        for request in ('auto', 'map', 'lsmr', 'lsmr-reference'):
            native_io.validate_ols_request(raw, request)
        for field, value in ((0, 1), (3, 1), (9, 1), (10, 1), (13, 1), (14, 1), (15, .1)):
            changed = copy.deepcopy(raw)
            set_row(changed, 0, {field: value})
            for request in ('map', 'lsmr'):
                with self.subTest(field=field, request=request), self.assertRaises(ValueError):
                    native_io.validate_ols_request(changed, request)

    def test_worker_route_warning_local_and_pilot_decisions_stay_exact(self):
        raw = current_result()
        signature = qualify.worker_signature(raw)
        for fields in ({0: 2}, {1: 3}, {3: 3, 4: 5}, {9: 1}, {11: 1, 12: 3}, {13: 1}, {14: 1}):
            changed = copy.deepcopy(raw)
            set_row(changed, 1, fields)
            with self.subTest(fields=fields):
                self.assertNotEqual(signature, qualify.worker_signature(changed))
        for fields in ({10: 11}, {15: .1}, {19: 2e-15}):
            changed = copy.deepcopy(raw)
            set_row(changed, 1, fields)
            self.assertEqual(signature, qualify.worker_signature(changed))

    def test_worker_certificate_decisions_are_protected_but_magnitudes_are_not(self):
        raw = current_result()
        signature = qualify.worker_signature(raw)
        for index in (1, 3, 5, 7, 47, 49, 51, 53, 57, 59):
            changed = copy.deepcopy(raw)
            changed['matrices']['RCERT']['bits'][index] = bits(2.)
            self.assertNotEqual(signature, qualify.worker_signature(changed))
        incomplete = copy.deepcopy(raw)
        incomplete['matrices']['RCERT']['bits'][63] = bits(0.)
        with self.assertRaises(ValueError):
            qualify.worker_signature(incomplete)
        for index in (9, 11, 45, 55, 61):
            changed = copy.deepcopy(raw)
            changed['matrices']['RCERT']['bits'][index] = bits(2.)
            self.assertEqual(signature, qualify.worker_signature(changed))

    def test_current_ordinary_comparison_never_uses_historical_route_waivers(self):
        before, after = current_result(), current_result()
        set_row(after, 0, {1: 2, 3: 0})
        with patch.object(science.cmgschur, 'scientific_comparison'), self.assertRaisesRegex(ValueError, 'unaffected'):
            science.compatibility(before, after, None, None, {}, 'auto')

    def test_derived_final_certificate_uses_the_requested_target(self):
        raw = derived()
        raw['matrices']['RRUNTIME']['bits'][45] = bits(1e-10)
        set_row(raw, 1, {19: 1e-12})
        science.validate_current_projection(raw, meta())
        with self.assertRaises(ValueError):
            science.validate_current_projection(raw, meta(residual=5))
        too_large = copy.deepcopy(raw)
        set_row(too_large, 1, {19: 2e-10})
        with self.assertRaises(ValueError):
            science.validate_current_projection(too_large, meta())
        projected_row = projected()
        projected_row['matrices']['RRUNTIME']['bits'][45] = bits(1e-10)
        set_row(projected_row, 1, {19: 1e-12})
        with self.assertRaises(ValueError):
            science.validate_current_projection(projected_row, meta())

    def test_derived_row_requires_fast_complete_result_and_exact_outside(self):
        before, after = projected(), derived()
        for label, mutate in (
                ('low', lambda r: r['matrices']['RRUNTIME']['bits'].__setitem__(3, bits(2.))),
                ('uncertified', lambda r: r['matrices']['RCERT']['bits'].__setitem__(-1, bits(0.))),
                ('pilot contraction', lambda r: set_row(r, 1, {15: .1})),
                ('warnings', lambda r: set_row(r, 1, {13: 1}))):
            changed = copy.deepcopy(after)
            mutate(changed)
            with self.subTest(label=label), self.assertRaises(ValueError):
                science.derived_residual_only(before, changed, meta())
        for table, index in (('RRUNTIME', 59), ('RDOF', 23), ('RMARKB', 0), ('RSOLVER', 10)):
            changed = copy.deepcopy(after)
            changed['matrices'][table]['bits'][index] = bits(2.)
            with self.subTest(table=table):
                self.assertFalse(science.derived_residual_only(before, changed, meta()))
