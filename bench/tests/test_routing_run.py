"""Actual producer adapter witnesses, with native calls replaced by tiny fixtures."""
import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_run
from native_io import case_header, digest
from test_cmgschur import bits, case_bytes, result
from test_routing import h, manifest, records, prefix_manifest, overlap_manifest, portfolio_manifest

TRACE = ('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_h4_k0|final=0|certified=1|iterations=5|native_limit=100|'
         'setup_work=12|setup_limit=32|memory_bytes=64|memory_limit=128\n')


class ExplicitComparatorScope(unittest.TestCase):
    @staticmethod
    def raw(arm, mode=1, poolsize=1, capacity=1):
        raw = result(arm)
        raw['matrices']['RRUNTIME']['bits'][3] = bits(float(mode))
        raw['matrices']['RRUNTIME']['bits'][5] = bits(float(poolsize))
        raw['matrices']['RSTATUS'] = {'rows': capacity, 'columns': 4, 'bits': [bits(1.)] * (4 * capacity)}
        solver = raw['matrices']['RSOLVER']['bits']
        for row in range(2):
            for column, value in {3: 2., 10: 0., 11: 0., 12: 1. if arm == 'lsmr' else 0.,
                                  13: 3. if arm == 'lsmr' else 0., 14: 0., 15: 0.}.items():
                solver[row * 16 + column] = bits(value)
        return raw

    def compare(self, raw, arm, meta=None):
        # Numerical science is checked separately; exercise the exact diagnostic
        # scope here without a native process or fabricated scientific witness.
        runtime = routing_run.cmgschur.numbers(raw['matrices']['RRUNTIME'])
        reference = self.raw('auto', runtime[3], runtime[5], raw['matrices']['RSTATUS']['rows'])
        with patch.object(routing_run.cmgschur, 'scientific_comparison'):
            routing_run.compatibility(reference, raw, None, None, {} if meta is None else meta, arm)

    def correction(self):
        raw = self.raw('cmg')
        for column, value in {1: 4., 10: 1., 11: 1.}.items():
            raw['matrices']['RSOLVER']['bits'][16 + column] = bits(value)
        return raw

    def test_only_typed_cmg_final_correction_adds_two_exact_scope_leaves(self):
        ordinary, corrected = self.raw('cmg'), self.correction()
        before = set(routing_run.replay_scope(ordinary, 'cmg'))
        after = set(routing_run.replay_scope(corrected, 'cmg'))
        self.assertEqual(after - before, {'/matrices/RSOLVER/bits/22', '/matrices/RSOLVER/bits/23'})
        self.assertFalse(before - after)
        self.compare(ordinary, 'cmg')
        self.compare(corrected, 'cmg')
        # Fast final verification may certify an entrywise-zero system without
        # a CG update; its one verification/correction warning is not polishing.
        corrected['matrices']['RSOLVER']['bits'][18] = bits(0.)
        self.compare(corrected, 'cmg')
        # Primary polishing has a separately asserted mode-aware contract.
        ordinary['matrices']['RSOLVER']['bits'][11] = bits(1.)
        self.compare(ordinary, 'cmg')

    def test_cmg_final_mapping_rejects_other_solver_fallback_stop_and_preconditioner_changes(self):
        mutations = [(0, 1, 4.), (1, 0, 1.), (1, 1, 3.), (1, 1, 7.), (1, 3, 3.),
                     (1, 10, 0.), (1, 11, 0.), (1, 11, 2.), (1, 12, 1.),
                     (1, 13, 3.), (1, 14, 1.), (1, 15, .1)]
        for row, column, value in mutations:
            raw = self.correction()
            raw['matrices']['RSOLVER']['bits'][16 * row + column] = bits(value)
            with self.subTest(row=row, column=column, value=value), self.assertRaises(ValueError):
                self.compare(raw, 'cmg')
        for arm in ['auto', 'map', 'it', 'lsmr', 'cmgschur5']:
            raw = self.raw(arm)
            for column, value in {1: 4., 10: 1., 11: 1.}.items():
                raw['matrices']['RSOLVER']['bits'][16 + column] = bits(value)
            with self.subTest(arm=arm), self.assertRaises(ValueError):
                self.compare(raw, arm)
        for table, index in [('RRUNTIME', 43), ('RCERT', 1)]:
            raw = self.correction()
            raw['matrices'][table]['bits'][index] = bits(2.)
            with self.subTest(table=table), self.assertRaisesRegex(ValueError, 'unaffected field'):
                self.compare(raw, 'cmg')

    def test_polishing_warning_bounds_follow_mode_and_all_coefficient_slots(self):
        for mode, capacity, poolsize, limits in [(1, 9, 2, (1, 0)), (2, 9, 2, (6, 1)), (2, 1, 10, (2, 1))]:
            for primary in range(limits[0] + 1):
                for final in range(limits[1] + 1):
                    raw = self.raw('cmg', mode, poolsize, capacity)
                    # Low merges max(iterations) but concatenates warnings.
                    raw['matrices']['RSOLVER']['bits'][11] = bits(float(primary))
                    raw['matrices']['RSOLVER']['bits'][27] = bits(float(final))
                    with self.subTest(mode=mode, primary=primary, final=final):
                        self.assertEqual(routing_run.cmg_polish_limits(raw), (mode, limits))
                        self.compare(raw, 'cmg')
            raw = self.raw('cmg', mode, poolsize, capacity)
            # Omitted/aliased status does not reduce the pre-QR projection bound.
            raw['matrices']['RSTATUS']['bits'][1::4] = [bits(2.)] * capacity
            self.assertEqual(routing_run.cmg_polish_limits(raw), (mode, limits))

    def test_polishing_rejects_invalid_counts_modes_and_nonpolishing_tuples(self):
        for mode, row, count in [(1, 0, 2.), (1, 1, 1.), (2, 0, 4.), (2, 1, 2.),
                                  (1, 0, -1.), (1, 0, .5), (1, 0, float('nan')), (1, 0, float('inf'))]:
            raw = self.raw('cmg', mode, 2, 3)  # Low bound: 1 + ceil(3/2) = 3.
            raw['matrices']['RSOLVER']['bits'][16 * row + 11] = bits(count)
            with self.subTest(mode=mode, row=row, count=count), self.assertRaises(ValueError):
                routing_run.replay_scope(raw, 'cmg')
        for row, column, value in [(0, 0, 1.), (0, 1, 4.), (0, 3, 1.), (0, 10, 1.),
                                   (0, 12, 1.), (0, 13, 1.), (0, 14, 1.), (0, 15, .1),
                                   (1, 10, 1.), (1, 14, 1.), (1, 15, .1), (0, 2, 0.)]:
            raw = self.raw('cmg')
            raw['matrices']['RSOLVER']['bits'][11] = bits(1.)
            raw['matrices']['RSOLVER']['bits'][16 * row + column] = bits(value)
            with self.subTest(row=row, column=column), self.assertRaises(ValueError):
                routing_run.replay_scope(raw, 'cmg')
        for index, value in [(2, 0.), (3, 0.), (3, 3.), (3, 1.5), (3, float('nan')),
                             (4, 0.), (5, 0.), (5, -1.), (5, .5), (5, float('inf'))]:
            raw = self.raw('cmg')
            raw['matrices']['RRUNTIME']['bits'][index] = bits(value)
            with self.subTest(runtime_index=index, value=value), self.assertRaises(ValueError):
                routing_run.replay_scope(raw, 'cmg')
        raw = self.correction()
        raw['matrices']['RRUNTIME']['bits'][3] = bits(2.)
        with self.assertRaisesRegex(ValueError, 'final correction'):
            routing_run.replay_scope(raw, 'cmg')

    def test_polishing_scope_preserves_mode_pool_tolerance_certificates_and_other_families(self):
        for index, value in [(3, 2.), (5, 2.), (43, 2.)]:
            raw = self.raw('cmg')
            raw['matrices']['RRUNTIME']['bits'][index] = bits(value)
            with patch.object(routing_run.cmgschur, 'scientific_comparison'), self.assertRaisesRegex(ValueError, 'unaffected field'):
                routing_run.compatibility(self.raw('auto'), raw, None, None, {}, 'cmg')
        for arm in ['auto', 'map', 'it', 'lsmr', 'cmgschur5']:
            raw = self.raw(arm)
            raw['matrices']['RSOLVER']['bits'][11] = bits(1.)
            with self.subTest(arm=arm), self.assertRaises(ValueError):
                self.compare(raw, arm)
        raw = self.raw('cmg')
        raw['matrices']['RCERT']['bits'][1] = bits(0.)
        with self.assertRaisesRegex(ValueError, 'unaffected field'):
            self.compare(raw, 'cmg')


class PortfolioAutoScope(unittest.TestCase):
    META = {'categorical_intercepts': True, 'absorbed_terms': 3, 'rows': 2}

    @staticmethod
    def raw(route='lsmr', *, pilot=0, mode=1, warnings=None):
        raw = ExplicitComparatorScope.raw('auto', mode, 1, 3)
        runtime = raw['matrices']['RRUNTIME']['bits']
        for column, value in {43: 1e-8, 45: 1e-8, 47: 100.}.items():
            runtime[column] = bits(value)
        used, fallback, requested_pc, used_pc = {
            'prefix': (2, 0, 0, 0), 'map-no-lsmr': (2, 0, 0, 0),
            'lsmr': (3, 0, 1, 4), 'map-fallback': (2, 1, 1, 4),
            'pc-fallback': (3, 1, 1, 2)}[route]
        for row in range(2):
            fields = {0: 1, 1: used, 2: pilot if route == 'prefix' else 10,
                      3: 2, 4: 0., 5: 1e-15, 6: 1e-15, 7: 1e-15,
                      10: fallback, 11: fallback if warnings is None else warnings,
                      12: requested_pc, 13: used_pc, 14: pilot, 15: 0.}
            for column, value in fields.items():
                raw['matrices']['RSOLVER']['bits'][row * 16 + column] = bits(float(value))
        return raw

    def compare(self, raw, *, reference=None, meta=None, policy=None, reader='auto'):
        reference = self.raw(mode=int(routing_run.cmgschur.numbers(raw['matrices']['RRUNTIME'])[3])) if reference is None else reference
        with patch.object(routing_run.cmgschur, 'scientific_comparison') as science:
            routing_run.compatibility(reference, raw, None, None,
                self.META if meta is None else meta, reader,
                selected_policy=routing.policy(routing.PORTFOLIO_V1) if policy is None else policy)
            science.assert_called_once()

    def test_only_named_portfolio_leaves_follow_valid_actual_routes(self):
        for route, pilot in [('prefix', 0), ('prefix', 2), ('prefix', 4), ('lsmr', 4),
                             ('lsmr', 0), ('lsmr', 1), ('map-no-lsmr', 0),
                             ('map-fallback', 4), ('pc-fallback', 4)]:
            raw = self.raw(route, pilot=pilot)
            before = copy.deepcopy(raw)
            with self.subTest(route=route, pilot=pilot):
                self.compare(raw)
                scope = set(routing_run.portfolio_scope(self.raw(), raw, self.META, 'auto',
                            routing.policy(routing.PORTFOLIO_V1)))
                old = set(routing_run.replay_scope(raw, 'auto', self.META))
                self.assertEqual(scope - old, {f'/matrices/RSOLVER/bits/{14*r+c}'
                    for r in range(2) for c in [1, 8, 9, 10, 11, 12]})
                self.assertEqual(raw, before)

    def test_low_merge_preserves_concrete_route_and_bounded_warning_counts(self):
        raw = self.raw('map-fallback', pilot=4, mode=2)
        raw['matrices']['RSOLVER']['bits'][11] = bits(4.)  # outcome + three chunks
        self.compare(raw)
        raw = self.raw('lsmr', pilot=4, mode=2)  # MAP-only chunks + LSMR chunk
        self.compare(raw)
        raw['matrices']['RSOLVER']['bits'][11] = bits(29.)  # bound is 4*(2*3+1)
        with self.assertRaisesRegex(ValueError, 'warning tuple'):
            self.compare(raw)

    def test_portfolio_rejects_forged_routes_and_loose_certificates(self):
        mutations = [(0, 3.), (1, 4.), (3, 3.), (10, 1.), (11, .5), (11, -1.),
                     (12, 0.), (13, 0.), (14, 5.), (14, 11.), (15, .1),
                     (5, 2e-10), (2, 101.), (2, float('nan'))]
        for row in range(2):
            for column, value in mutations:
                raw = self.raw('lsmr', pilot=4)
                raw['matrices']['RSOLVER']['bits'][row * 16 + column] = bits(value)
                with self.subTest(row=row, column=column), self.assertRaises(ValueError):
                    self.compare(raw)
        for route, column, value in [('prefix', 11, 1.), ('map-fallback', 10, 0.),
                                      ('map-fallback', 11, 0.), ('pc-fallback', 13, 4.)]:
            raw = self.raw(route, pilot=2)
            raw['matrices']['RSOLVER']['bits'][column] = bits(value)
            with self.subTest(route=route, column=column), self.assertRaises(ValueError):
                self.compare(raw)
        raw = self.raw('pc-fallback', pilot=4)
        raw['matrices']['RSOLVER']['bits'][12] = bits(2.)
        with self.assertRaisesRegex(ValueError, 'preconditioner'):
            self.compare(raw)  # An explicit Off request is not the prepared Auto request.
        raw = self.raw()
        raw['matrices']['RSOLVER']['bits'][21] = bits(2e-14)
        with self.assertRaisesRegex(ValueError, 'private Auto target'):
            self.compare(raw)

    def test_portfolio_never_scopes_unaffected_models_or_ordinary_auto(self):
        raw = self.raw('prefix', pilot=2)
        for meta in [{**self.META, 'absorbed_terms': n} for n in [0, 1, 2]] + [
                {**self.META, 'categorical_intercepts': False}]:
            with self.subTest(meta=meta), self.assertRaises(ValueError):
                self.compare(raw, meta=meta)
        with patch.object(routing_run.cmgschur, 'scientific_comparison'), self.assertRaisesRegex(ValueError, 'unaffected field'):
            routing_run.compatibility(self.raw(), raw, None, None, self.META, 'auto')
        for policy in [routing.policy('prefix-map4'), routing.policy('h16-k0'),
                       {**routing.policy(routing.PORTFOLIO_V1), 'certificate': 'requested'}]:
            with self.subTest(policy=policy), self.assertRaises(ValueError):
                self.compare(raw, policy=policy)
        with self.assertRaises(ValueError):
            self.compare(raw, reader='lsmr')
        for table, index in [('RDOF', 3), ('RCERT', 1), ('RRUNTIME', 43), ('RSTATUS', 1)]:
            changed = self.raw('prefix', pilot=2)
            changed['matrices'][table]['bits'][index] = bits(2.)
            with self.subTest(table=table), self.assertRaises(ValueError):
                self.compare(changed)

    def test_two_fe_portfolio_preserves_the_existing_routing_scope(self):
        raw = self.raw()
        meta = {**self.META, 'absorbed_terms': 2}
        selected = routing.policy(routing.PORTFOLIO_V1)
        self.assertEqual(routing_run.portfolio_scope(raw, raw, meta, 'auto', selected),
                         routing_run.cross_request_scope(raw, raw, 'auto', meta))
        # Changed native iteration work already belongs to the old h/k scope.
        changed = copy.deepcopy(raw)
        changed['matrices']['RSOLVER']['bits'][2] = bits(12.)
        self.compare(changed, reference=raw, meta=meta)
        for field, value in [('checkpoint', 16.0), ('trees', True), ('trees', 2),
                             ('canonical_cache', 1)]:
            forged = copy.deepcopy(selected)
            forged['two_fe'][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.compare(raw, meta=meta, policy=forged)

    def test_portfolio_binding_rejects_wrong_manifest_candidate_and_policy(self):
        spec = portfolio_manifest()
        self.assertEqual(routing_run.comparison_policy(spec, routing.PORTFOLIO_V1),
                         routing.policy(routing.PORTFOLIO_V1))
        self.assertIsNone(routing_run.comparison_policy(spec, 'baseline'))
        bad = copy.deepcopy(spec)
        bad['arms'][routing.PORTFOLIO_V1]['policy']['two_fe']['trees'] = True
        with self.assertRaises(ValueError):
            routing_run.comparison_policy(bad, routing.PORTFOLIO_V1)
        for field, value in [('experiment_kind', 'routing'), ('candidates', ['baseline'])]:
            bad = copy.deepcopy(spec)
            bad[field] = value
            with self.assertRaises(ValueError):
                routing_run.comparison_policy(bad, routing.PORTFOLIO_V1)

    def test_two_fe_direct_changes_only_the_validated_family_leaf(self):
        meta = {**self.META, 'absorbed_terms': 2}
        before, after = self.raw('map-no-lsmr'), self.raw('prefix')
        for row in range(2):
            before['matrices']['RSOLVER']['bits'][16 * row + 1] = bits(4.)
        # Primary direct certification uses the requested, not private, target.
        after['matrices']['RSOLVER']['bits'][5] = bits(5e-9)
        self.compare(after, reference=before, meta=meta)
        scope = set(routing_run.portfolio_scope(before, after, meta, 'auto', routing.policy(routing.PORTFOLIO_V1)))
        self.assertEqual(scope - set(routing_run.replay_scope(after, 'auto', meta)),
                         {'/matrices/RSOLVER/bits/1', '/matrices/RSOLVER/bits/15'})
        for which, row, column, value in [('after', 0, 2, 1.), ('after', 0, 5, 2e-8),
                ('after', 1, 5, 2e-14), ('before', 0, 1, 3.), ('before', 0, 14, 1.)] + [
                ('after', row, column, 1.) for row in range(2) for column in [10, 11, 12, 13, 14, 15]]:
            a, b = copy.deepcopy(before), copy.deepcopy(after)
            (a if which == 'before' else b)['matrices']['RSOLVER']['bits'][16 * row + column] = bits(value)
            with self.subTest(which=which, row=row, column=column), self.assertRaises(ValueError):
                self.compare(b, reference=a, meta=meta)
        for column in [43, 45, 47]:
            changed = copy.deepcopy(after)
            changed['matrices']['RRUNTIME']['bits'][column] = bits(2.)
            with self.subTest(runtime=column), self.assertRaises(ValueError):
                self.compare(changed, reference=before, meta=meta)

    def test_activated_portfolio_policy_is_bound_to_c_and_keeps_equivalence_exact(self):
        import routing_block_deployment as deployment
        selected = routing.policy(routing.PORTFOLIO_V1)
        spec = {'kind': deployment.KIND, 'purpose': 'public_default_holdout', 'panels': {'C': []},
            'candidates': ['public-auto'], 'protections': ['baseline', 'incumbent'], 'arms': {
                'baseline': {'policy': None},
                'incumbent': {'role': 'required_reference', 'policy': copy.deepcopy(selected)},
                'public-auto': {'role': 'candidate', 'solver': 'auto', 'policy': None,
                    'invocation': {'solver': 'auto', 'injection': None}, 'effective_policy': selected}}}
        for arm in ['public-auto', 'incumbent']:
            self.assertEqual(routing_run.comparison_policy(spec, arm), selected)
        self.assertIsNone(routing_run.comparison_policy(spec, 'baseline'))
        for key, value in [('purpose', 'activation_equivalence'), ('panels', {'B': []}),
                           ('candidates', ['incumbent']), ('protections', ['baseline'])]:
            bad = copy.deepcopy(spec); bad[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                routing_run.comparison_policy(bad, 'public-auto')
        for arm, key, value in [('public-auto', 'invocation', {'solver': 'auto', 'injection': 'x'}),
                ('public-auto', 'policy', selected), ('public-auto', 'solver', 'map'),
                ('incumbent', 'role', 'candidate'), ('incumbent', 'policy', routing.policy('h16-k1'))]:
            bad = copy.deepcopy(spec); bad['arms'][arm][key] = value
            with self.subTest(arm=arm, key=key), self.assertRaises(ValueError):
                routing_run.comparison_policy(bad, 'public-auto')
        equivalent = copy.deepcopy(spec)
        equivalent.update(kind=deployment.EQUIVALENCE, purpose='activation_equivalence', panels={'A': []})
        for arm in ['public-auto', 'incumbent']:
            self.assertIsNone(routing_run.comparison_policy(equivalent, arm))
        with patch.object(routing_run.cmgschur, 'scientific_comparison'), self.assertRaises(ValueError):
            routing_run.compatibility(self.raw(), self.raw('prefix', pilot=2), None, None, self.META, 'auto',
                selected_policy=routing_run.comparison_policy(equivalent, 'public-auto'))


class CrossRequestComparatorScope(unittest.TestCase):
    META = {'categorical_intercepts': True, 'absorbed_terms': 3}

    @staticmethod
    def pair(arm):
        reference = ExplicitComparatorScope.raw('auto')
        raw = ExplicitComparatorScope.raw(arm)
        for column, value in {0: 1., 1: 2., 2: 4., 3: 2., 10: 1., 11: 1.,
                              12: 1., 13: 3., 14: 0., 15: 0.}.items():
            reference['matrices']['RSOLVER']['bits'][column] = bits(value)
        return reference, raw

    def compare(self, reference, raw, arm, meta=None):
        with patch.object(routing_run.cmgschur, 'scientific_comparison'):
            routing_run.compatibility(reference, raw, None, None,
                                      self.META if meta is None else meta, arm)

    def test_only_authenticated_primary_cross_request_adds_two_leaves(self):
        for arm in ['lsmr', 'map', 'it']:
            for preconditioner in [2., 3., 4.]:
                with self.subTest(arm=arm, preconditioner=preconditioner):
                    reference, raw = self.pair(arm)
                    reference['matrices']['RSOLVER']['bits'][13] = bits(preconditioner)
                    scope = set(routing_run.cross_request_scope(reference, raw, arm, self.META))
                    previous = set(routing_run.replay_scope(raw, arm, self.META))
                    self.assertEqual(scope - previous,
                        {'/matrices/RSOLVER/bits/8', '/matrices/RSOLVER/bits/9'})
                    self.assertFalse(previous - scope)
                    self.compare(reference, raw, arm)

    def test_reference_and_target_mapping_mutations_cannot_receive_allowance(self):
        reference_changes = [(0, 2.), (1, 4.), (2, 0.), (2, -1.), (2, .5),
            (2, float('nan')), (2, float('inf')), (3, 1.), (10, 0.), (11, 2.),
            (12, 0.), (13, 0.), (14, 1.), (15, .1)]
        target_changes = [(0, 1.), (1, 4.), (2, 0.), (2, -1.), (2, .5),
            (2, float('nan')), (2, float('inf')), (3, 1.), (10, 1.), (11, 1.),
            (12, 2.), (13, 1.), (14, 1.), (15, .1)]
        for arm in ['lsmr', 'map', 'it']:
            for side, changes in [('reference', reference_changes), ('target', target_changes)]:
                for column, value in changes:
                    with self.subTest(arm=arm, side=side, column=column, value=value):
                        reference, raw = self.pair(arm)
                        changed = reference if side == 'reference' else raw
                        changed['matrices']['RSOLVER']['bits'][column] = bits(value)
                        with self.assertRaises(ValueError):
                            self.compare(reference, raw, arm)

    def test_return_codes_shapes_and_final_diagnostics_remain_protected(self):
        for arm in ['lsmr', 'map', 'it']:
            for side in ['reference', 'target']:
                for mutation in ['return', 'bool return', 'rows', 'columns', 'bits', 'bool rows']:
                    with self.subTest(arm=arm, side=side, mutation=mutation):
                        reference, raw = self.pair(arm)
                        changed = reference if side == 'reference' else raw
                        solver = changed['matrices']['RSOLVER']
                        if mutation in ['return', 'bool return']:
                            changed['return_code'] = False if mutation == 'bool return' else 459
                        elif mutation == 'bits':
                            solver['bits'].pop()
                        else:
                            field = 'rows' if mutation == 'bool rows' else mutation
                            solver[field] = True if mutation == 'bool rows' else 1
                        with self.assertRaises(ValueError):
                            self.compare(reference, raw, arm)
            for column in [10, 11]:
                reference, raw = self.pair(arm)
                raw['matrices']['RSOLVER']['bits'][16 + column] = bits(1.)
                with self.subTest(arm=arm, final_column=column), self.assertRaises(ValueError):
                    self.compare(reference, raw, arm)

    def test_auto_same_request_and_generic_it_get_no_cross_request_scope(self):
        for arm in ['auto', 'lsmr', 'map', 'it']:
            reference = ExplicitComparatorScope.raw(arm)
            raw = copy.deepcopy(reference)
            scope = routing_run.cross_request_scope(reference, raw, arm, self.META)
            self.assertEqual(scope, routing_run.replay_scope(raw, arm, self.META))
            raw['matrices']['RSOLVER']['bits'][10] = bits(1.)
            with self.subTest(same_request=arm), self.assertRaises(ValueError):
                self.compare(reference, raw, arm)
        reference, raw = self.pair('it')
        for meta in [{}, {'categorical_intercepts': False, 'absorbed_terms': 3},
                     {'categorical_intercepts': True, 'absorbed_terms': True}]:
            with self.subTest(meta=meta), self.assertRaises(ValueError):
                self.compare(reference, raw, 'it', meta)
        generic = GenericItComparatorScope.raw()
        reference = ExplicitComparatorScope.raw('auto', 1, 10, 3)
        for column, value in {1: 2., 10: 1., 11: 1., 12: 1., 13: 3.}.items():
            reference['matrices']['RSOLVER']['bits'][column] = bits(value)
        with self.assertRaises(ValueError):
            self.compare(reference, generic, 'it', GenericItComparatorScope.META)

    def test_scientific_comparison_failure_still_precedes_scope(self):
        reference, raw = self.pair('lsmr')
        with patch.object(routing_run.cmgschur, 'scientific_comparison',
                          side_effect=ValueError('independent science failed')) as science:
            with self.assertRaisesRegex(ValueError, 'independent science failed'):
                routing_run.compatibility(reference, raw, None, None, self.META, 'lsmr')
            science.assert_called_once_with(reference, raw, None, None, self.META)


class GenericItComparatorScope(unittest.TestCase):
    META = {'categorical_intercepts': False, 'absorbed_terms': 2}

    @staticmethod
    def raw(mode=1, poolsize=10, capacity=3, primary=1):
        raw = ExplicitComparatorScope.raw('it', mode, poolsize, capacity)
        raw['matrices']['RSOLVER']['bits'][11] = bits(float(primary))
        raw['matrices']['RSOLVER']['bits'][27] = bits(1.)
        return raw

    raw_reference = staticmethod(ExplicitComparatorScope.raw)

    def test_only_generic_it_warning_leaves_change_with_source_bounded_counts(self):
        for mode, capacity, poolsize, limit in [(1, 9, 2, 1), (2, 9, 2, 6), (2, 3, 10, 2)]:
            for count in range(1, limit + 1):
                raw = self.raw(mode, poolsize, capacity, count)
                before = set(routing_run.replay_scope(raw, 'it'))
                after = set(routing_run.replay_scope(raw, 'it', self.META))
                self.assertEqual(after - before, {'/matrices/RSOLVER/bits/9', '/matrices/RSOLVER/bits/23'})
                self.assertFalse(before - after)
                self.assertEqual(routing_run.generic_it_warning_limits(raw, self.META), (limit, 1))
                reference = self.raw_reference('auto', mode, poolsize, capacity)
                with patch.object(routing_run.cmgschur, 'scientific_comparison'):
                    routing_run.compatibility(reference, raw, None, None, self.META, 'it')
                raw['matrices']['RSOLVER']['bits'][2] = bits(0.)
                raw['matrices']['RSOLVER']['bits'][18] = bits(0.)
                # Initial certification still emits one warning per action.
                routing_run.replay_scope(raw, 'it', self.META)
                # Omission status never substitutes retained rank for capacity.
                raw['matrices']['RSTATUS']['bits'][1::4] = [bits(2.)] * capacity
                self.assertEqual(routing_run.generic_it_warning_limits(raw, self.META), (limit, 1))

    def test_categorical_missing_or_nonabsorbed_metadata_cannot_relax_it_warnings(self):
        for meta in [{}, {'family': 'slopes'}, {'categorical_intercepts': True, 'absorbed_terms': 2},
                     {'categorical_intercepts': 0, 'absorbed_terms': 2},
                     {'categorical_intercepts': False, 'absorbed_terms': 0},
                     {'categorical_intercepts': False, 'absorbed_terms': True}]:
            raw = self.raw()
            with self.subTest(meta=meta), patch.object(routing_run.cmgschur, 'scientific_comparison'), \
                    self.assertRaisesRegex(ValueError, 'unaffected field'):
                routing_run.compatibility(self.raw_reference('auto', 1, 10, 3), raw, None, None, meta, 'it')
        for arm in ['auto', 'map', 'lsmr', 'cmgschur5']:
            raw = self.raw_reference(arm)
            raw['matrices']['RSOLVER']['bits'][11] = bits(1.)
            with self.subTest(arm=arm), patch.object(routing_run.cmgschur, 'scientific_comparison'), \
                    self.assertRaises(ValueError):
                routing_run.compatibility(self.raw_reference('auto'), raw, None, None, self.META, arm)

    def test_it_rejects_changed_tuples_and_invalid_warning_or_iteration_counts(self):
        for row in range(2):
            for column, value in [(0, 1.), (1, 2.), (3, 1.), (10, 1.), (12, 1.), (13, 3.),
                                  (14, 1.), (15, .1), (2, -1.), (2, .5), (2, float('nan'))]:
                raw = self.raw()
                raw['matrices']['RSOLVER']['bits'][row * 16 + column] = bits(value)
                with self.subTest(row=row, column=column, value=value), self.assertRaises(ValueError):
                    routing_run.replay_scope(raw, 'it', self.META)
            for count in [0., -1., .5, 2., float('nan'), float('inf')]:
                raw = self.raw()
                raw['matrices']['RSOLVER']['bits'][row * 16 + 11] = bits(count)
                with self.subTest(row=row, count=count), self.assertRaises(ValueError):
                    routing_run.replay_scope(raw, 'it', self.META)
        raw = self.raw(2, 2, 3, 4)  # Capacity bound is 1 + ceil(3/2) = 3.
        with self.assertRaises(ValueError):
            routing_run.replay_scope(raw, 'it', self.META)

    def test_it_rejects_invalid_execution_metadata_and_shapes(self):
        for index, value in [(2, 0.), (3, 0.), (3, 3.), (3, 1.5), (3, float('nan')),
                             (4, 0.), (5, 0.), (5, -1.), (5, .5), (5, float('inf'))]:
            raw = self.raw()
            raw['matrices']['RRUNTIME']['bits'][index] = bits(value)
            with self.subTest(index=index, value=value), self.assertRaises(ValueError):
                routing_run.replay_scope(raw, 'it', self.META)
        for table, field, value in [('RRUNTIME', 'rows', 29), ('RSOLVER', 'rows', 1),
                                    ('RSOLVER', 'columns', 15), ('RSTATUS', 'rows', 0),
                                    ('RSTATUS', 'rows', True), ('RSTATUS', 'columns', 3)]:
            raw = self.raw()
            raw['matrices'][table][field] = value
            with self.subTest(table=table, field=field), self.assertRaises(ValueError):
                routing_run.replay_scope(raw, 'it', self.META)
        raw = self.raw()
        raw['return_code'] = 459
        with self.assertRaises(ValueError):
            routing_run.replay_scope(raw, 'it', self.META)

    def test_it_scope_keeps_unrelated_fields_and_raw_repeat_warning_bits(self):
        for table, index, value in [('RRUNTIME', 3, 2.), ('RRUNTIME', 5, 2.),
                                    ('RRUNTIME', 43, 2.), ('RCERT', 1, 0.)]:
            raw = self.raw()
            raw['matrices'][table]['bits'][index] = bits(value)
            with self.subTest(table=table, index=index), patch.object(routing_run.cmgschur, 'scientific_comparison'), \
                    self.assertRaisesRegex(ValueError, 'unaffected field'):
                routing_run.compatibility(self.raw_reference('auto', 1, 10, 3), raw, None, None, self.META, 'it')
        raw = self.raw(2, primary=2)
        before = copy.deepcopy(raw)
        routing_run.replay_scope(raw, 'it', self.META)
        self.assertEqual(raw, before)
        changed = copy.deepcopy(raw)
        changed['matrices']['RSOLVER']['bits'][11] = bits(1.)
        self.assertEqual(routing_run.differences(routing_run.exact_view(raw, False),
                                              routing_run.exact_view(changed, False)),
                         ['/matrices/RSOLVER/bits/9'])


class CmgWorkerDiagnostics(unittest.TestCase):
    @staticmethod
    def fixture(change=None):
        value = {'warmups': 0, 'repetitions': 1, 'cmg_worker_requests': [], 'prerequisites': [], 'records': []}
        for worker in [1, 4]:
            cid = f'case-t{worker}'
            raw = ExplicitComparatorScope.raw('cmg')
            if worker == 4 and change == 'warning':
                raw['matrices']['RSOLVER']['bits'][11] = bits(1.)
            if worker == 4 and change == 'fallback':
                for column, field in {1: 4., 10: 1., 11: 1.}.items():
                    raw['matrices']['RSOLVER']['bits'][16 + column] = bits(field)
            rows = routing_run.cmgschur.numbers(raw['matrices']['RSOLVER'])
            rows = [rows[:16], rows[16:]]
            value['cmg_worker_requests'].append({'cell': cid, 'arm': 'cmg', 'threads': worker, 'identity': h('same request')})
            value['prerequisites'].append({'cell': cid, 'arm': 'cmg', 'status': 'passed',
                                          'cmg_worker_signature': routing_run.cmg_diagnostic_signature(rows)})
            value['records'].append({'cell': cid, 'arm': 'cmg', 'status': 'passed', 'solver_rows': rows, 'repetition': 0})
        return value

    def test_worker_warning_or_final_fallback_mismatch_blocks_both_orders(self):
        for change in ['warning', 'fallback']:
            value = self.fixture(change)
            before = copy.deepcopy(value)
            first = routing_run.cmg_worker_checks(value)
            self.assertEqual(value, before)  # Guard does not rewrite observations.
            for key in ['cmg_worker_requests', 'prerequisites', 'records']:
                value[key].reverse()
            self.assertEqual(first, routing_run.cmg_worker_checks(value))
            self.assertEqual(first[1], {('case-t1', 'cmg'), ('case-t4', 'cmg')})
            self.assertEqual(first[0][0]['status'], 'blocked')
        self.assertEqual(routing_run.cmg_worker_checks(self.fixture())[1], set())

    def test_worker_identity_removes_only_threads_and_keeps_resource_environment(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'source.case'
            def key(threads, extra=b'memory=fast', arm='cmg', artifact=h('artifact'), limit=None):
                path.write_bytes(case_bytes([b'estimate-ols', b'solver=cmg', f'threads={threads}'.encode(), extra]))
                cell = {'threads': threads, 'body_sha256': h('same body')}
                if limit is not None:
                    cell['memory_limit_bytes'] = limit
                return routing_run.cmg_worker_identity(path, cell, arm, artifact)
            reference = key(1)
            self.assertEqual(reference, key(4))
            for extra in [b'memory=low', b'poolsize=2', b'tolerance=1e-12', b'inject=anything']:
                self.assertNotEqual(reference, key(4, extra))
            self.assertNotEqual(reference, key(4, arm='different-arm'))
            self.assertNotEqual(reference, key(4, artifact=h('different-artifact')))
            self.assertNotEqual(reference, key(4, limit=1000))
            cell = {'threads': 1, 'body_sha256': h('same body')}
            with self.assertRaisesRegex(ValueError, 'worker count'):
                routing_run.cmg_worker_identity(path, cell, 'cmg', h('artifact'))
        value = self.fixture('warning')
        value['cmg_worker_requests'][1]['identity'] = h('different effective request')
        checks, blocked = routing_run.cmg_worker_checks(value)
        self.assertEqual(blocked, set())
        self.assertTrue(all(check['status'] == 'not_compared' for check in checks))

    def test_warning_counts_remain_exact_within_repeats_and_incomplete_pairs_do_not_claim_comparison(self):
        value = self.fixture()
        value['records'][0]['solver_rows'][0][11] = 1.
        with self.assertRaisesRegex(ValueError, 'exact repeat'):
            routing_run.cmg_worker_checks(value)
        value = self.fixture()
        value['records'][0]['status'] = 'blocked'
        checks, blocked = routing_run.cmg_worker_checks(value)
        self.assertEqual(blocked, set())
        self.assertEqual(checks[0]['status'], 'not_compared')
        self.assertFalse(checks[0]['members'][0]['complete'])

    def test_agreeing_pair_does_not_hide_an_incomplete_planned_third_worker(self):
        value = self.fixture()
        value['cmg_worker_requests'].append({**value['cmg_worker_requests'][0], 'cell': 'case-t8', 'threads': 8})
        value['prerequisites'].append({'cell': 'case-t8', 'arm': 'cmg', 'status': 'blocked'})
        value['records'].append({'cell': 'case-t8', 'arm': 'cmg', 'status': 'not_run', 'repetition': 0})
        checks, blocked = routing_run.cmg_worker_checks(value)
        self.assertEqual(blocked, set())
        self.assertEqual(checks[0]['status'], 'not_compared')
        self.assertEqual(checks[0]['compared_worker_counts'], [1, 4])
        self.assertFalse(checks[0]['complete_coverage'])
        self.assertEqual(len(checks[0]['members']), 3)

    def test_planned_header_identity_matches_actual_preparation(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'source.case'
            source.write_bytes(case_bytes([b'estimate-ols', b'solver=auto', b'threads=1', b'preconditioner=auto',
                                           b'memory=low', b'poolsize=3', b'tolerance=1e-10', b'cmgsamples=2']))
            for worker in [1, 4]:
                cell = {'threads': worker, 'body_sha256': h('body'), 'memory_limit_bytes': 1000}
                planned = routing_run.cmg_worker_identity(source, cell, 'cmg', h('artifact'), prepared=False)
                prepared = Path(directory) / f'prepared-{worker}.case'
                routing_run.cmgschur.prepare(source, prepared, 'cmg', worker)
                self.assertEqual(planned, routing_run.cmg_worker_identity(prepared, cell, 'cmg', h('artifact')))


class GenericItWorkerDiagnostics(unittest.TestCase):
    @staticmethod
    def fixture(change=None):
        value = {'warmups': 0, 'repetitions': 1, 'generic_it_worker_requests': [],
                 'prerequisites': [], 'records': []}
        for worker in [1, 4]:
            cid = f'case-t{worker}'
            raw = GenericItComparatorScope.raw(2, primary=1 if worker == 4 and change == 'warning' else 2)
            rows = routing_run.cmgschur.numbers(raw['matrices']['RSOLVER'])
            rows = [rows[:16], rows[16:]]
            value['generic_it_worker_requests'].append({'cell': cid, 'arm': 'it', 'threads': worker,
                                                        'identity': h('same IT request')})
            value['prerequisites'].append({'cell': cid, 'arm': 'it', 'status': 'passed',
                'generic_it_worker_signature': routing_run.generic_it_diagnostic_signature(rows)})
            value['records'].append({'cell': cid, 'arm': 'it', 'status': 'passed',
                                     'solver_rows': rows, 'repetition': 0})
        return value

    def test_raw_warning_counts_stay_exact_across_workers_repeats_and_order(self):
        value = self.fixture('warning')
        before = copy.deepcopy(value)
        first = routing_run.generic_it_worker_checks(value)
        self.assertEqual(value, before)
        for key in ['generic_it_worker_requests', 'prerequisites', 'records']:
            value[key].reverse()
        self.assertEqual(first, routing_run.generic_it_worker_checks(value))
        self.assertEqual(first[1], {('case-t1', 'it'), ('case-t4', 'it')})
        self.assertEqual(first[0][0]['status'], 'blocked')
        value = self.fixture()
        self.assertEqual(routing_run.generic_it_worker_checks(value)[0][0]['status'], 'passed')
        value['records'][0]['solver_rows'][0][11] = 1.
        with self.assertRaisesRegex(ValueError, 'exact repeat'):
            routing_run.generic_it_worker_checks(value)
        value = self.fixture()
        value['records'][0]['status'] = 'blocked'
        check = routing_run.generic_it_worker_checks(value)[0][0]
        self.assertEqual(check['status'], 'not_compared')
        self.assertFalse(check['complete_coverage'])

    def test_it_worker_identity_matches_preparation_and_preserves_all_nonworker_options(self):
        with tempfile.TemporaryDirectory() as directory:
            source, prepared = Path(directory) / 'source.case', Path(directory) / 'prepared.case'
            def key(threads, extra=b'memory=low', artifact=h('artifact'), limit=None):
                source.write_bytes(case_bytes([b'estimate-ols', b'solver=auto', f'threads={threads}'.encode(), extra]))
                cell = {'threads': threads, 'body_sha256': h('same body')}
                if limit is not None:
                    cell['memory_limit_bytes'] = limit
                planned = routing_run.generic_it_worker_identity(source, cell, 'it', artifact, prepared=False)
                prepared.unlink(missing_ok=True)
                routing_run.cmgschur.prepare(source, prepared, 'it', threads)
                self.assertEqual(planned, routing_run.generic_it_worker_identity(prepared, cell, 'it', artifact))
                return planned
            reference = key(1)
            self.assertEqual(reference, key(4))
            for option in [b'memory=fast', b'poolsize=2', b'tolerance=1e-12', b'inject=anything']:
                self.assertNotEqual(reference, key(4, option))
            self.assertNotEqual(reference, key(4, artifact=h('changed artifact')))
            self.assertNotEqual(reference, key(4, limit=1000))


class RoutingProducer(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.spec = manifest()
        self.plugin = self.root / 'candidate'
        self.plugin.write_text('candidate')
        self.baseline = self.root / 'baseline'
        self.baseline.write_text('baseline')
        self.harness = self.root / 'harness'
        self.harness.write_text('harness')
        self.source = self.root / 'source.case'
        self.source.write_bytes(case_bytes())
        self.cell = self.spec['panels']['A'][0]
        self.cell['case_sha256'] = digest(self.source)
        self.meta = {'oracle': {}, 'rows': 2, 'columns': 6, 'body_sha256': self.cell['body_sha256'],
                     'absorbed_terms': 2,
                     'outputs': dict(zip(['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed'], range(6)))}
        self.data = np.array([[1., 1.], [.2, -.2], [0., 0.], [.3, .4], [.5, .2], [.2, -.2]])
        proof = self.root / 'proof'
        proof.write_text('independent test witness')
        self.witness = self.root / 'evidence.json'
        self.evidence = {'schema': 2, 'status': 'ready', 'case_sha256': self.cell['case_sha256'],
                         'body_sha256': self.cell['body_sha256'],
                         'manifest_sha256': routing.identity(self.spec), 'cell_id': self.cell['id'],
                         'threads': self.cell['threads'], 'arms': copy.deepcopy(self.spec['arms'])}
        for name in ['independent_witness', 'scoped_replay', 'resource_contract']:
            self.evidence[name] = {'path': proof.name, 'sha256': digest(proof)}
        self.witness.write_text(json.dumps(self.evidence))
        self.evidence['arm_outcomes'] = {a: {'status': 'passed'} for a in self.spec['arms']}
        self.plugins = {a: self.baseline if a == 'baseline' else self.plugin for a in self.spec['arms']}

    def run_panel(self, failure=None, ablation_only=False, prerequisites_only=False, witnesses=None, output_name='output', existing_proof=False, stage='A', previous=None):
        calls = []
        def native(command, log, timeout):
            traced = 'FEREG_SOLVER_TRACE=1' in command
            for name in routing_run.CLEARED_ENV:
                self.assertIn(['-u', name], [command[i:i+2] for i in range(len(command)-1)])
            with Path(command[-2]).open('rb') as stream:
                _, args = case_header(stream)
            fields = dict(a.decode().split('=', 1) for a in args[1:])
            arm = fields.get('inject', fields['solver'])
            calls.append((arm, traced))
            if failure == 'timeout' and arm == 'routing_h4_k0':
                raise TimeoutError('intentional bounded failure')
            raw = result(routing_run.EXPLICIT.get(fields['solver'], fields['solver']))
            for row in range(2):
                raw['matrices']['RSOLVER']['bits'][row * 16 + 12] = bits(1. if fields['solver'] == 'lsmr' else 0.)
                raw['matrices']['RSOLVER']['bits'][row * 16 + 13] = bits(3. if fields['solver'] == 'lsmr' else 0.)
            if self.spec.get('experiment_kind') == 'portfolio':
                candidate = arm == 'routing_portfolio_v1'
                raw = PortfolioAutoScope.raw('prefix' if candidate else 'lsmr', pilot=2 if candidate else 0)
                if fields['solver'] == 'lsmr':
                    for row in range(2):
                        raw['matrices']['RSOLVER']['bits'][16 * row] = bits(3.)
                if failure == 'portfolio-repeat' and candidate and not traced:
                    raw['matrices']['RSOLVER']['bits'][14] = bits(1.)
            if failure == 'repeat' and arm == 'routing_h4_k0' and not traced:
                raw['matrices']['RB']['bits'][0] = '3ff0000000000001'
            if failure == 'dof' and arm == 'routing_h4_k0':
                raw['matrices']['RDOF']['bits'][9] = '3ff0000000000001'
            if failure == 'tolerance' and arm == 'routing_h4_k0':
                raw['matrices']['RRUNTIME']['bits'][43] = '3ff0000000000001'
            if failure == 'used-engine' and arm == 'routing_h4_k0':
                raw['matrices']['RSOLVER']['bits'][1] = bits(3.)
            if failure == 'mutation' and not traced and arm == 'lsmr':
                self.baseline.write_text('mutated artifact')
            if failure == 'frozen-dependency-mutation' and not traced and arm == 'lsmr':
                self.bound_dependency.write_text('changed during native call')
            if arm == 'it' and failure in {'optional-exhaustion', 'optional-unknown', 'optional-resource-overrun'}:
                raw['return_code'] = 459
                raw['error'] = 'fereg: invalid_data: ' + ('borrowed Irons-Tuck exhausted its iteration budget without certification\n'
                    if failure != 'optional-unknown' else 'unexpected native failure\n')
            if failure == 'optional-wrong-science' and arm == 'it':
                raw['matrices']['RDOF']['bits'][9] = '3ff0000000000001'
            if arm == 'cmgschur' and (failure or '').startswith('optional-cmg-'):
                raw['return_code'] = 459 if failure == 'optional-cmg-wrong-code' else 430
                label = 'invalid_data' if failure == 'optional-cmg-wrong-label' else 'nonconvergence'
                message = 'unknown failure' if failure == 'optional-cmg-unknown' else routing.CMGSCHUR_NONCONVERGENCE
                raw['error'] = f'fereg: {label}: {message}\n'
            if failure == 'cohort-change':
                raw['matrices']['RB']['bits'][0] = '3ff0000000000001'
            if self.spec.get('purpose') == 'activation_equivalence':
                for row in range(2):
                    for column in [8, 9]:
                        raw['matrices']['RSOLVER']['bits'][16 * row + column] = bits(float(len(calls)))
            target = Path(command[-1])
            target.write_text(json.dumps({**raw, 'columns': []}))
            self.data.tofile(target.with_suffix('.json.columns'))
            trace = (TRACE.replace('preset=routing_h4_k0', 'preset=' + arm)
                     if traced and arm.startswith('routing_') and not (failure == 'missing-trace' and arm == 'routing_h4_k0') else '')
            if failure == 'optional-resource-overrun' and arm == 'it':
                trace += TRACE.replace('setup_work=12', 'setup_work=33')
            log.write_text('NATIVE_SECONDS=0.1\n' + trace)
            return {'wall_s': .1, 'maxrss_bytes': 10}
        current = json.loads(self.witness.read_text())
        if not prerequisites_only and not existing_proof:
            current.update(status='checked', stage='A', selection_sha256=None)
            current['arms'] = {a: current['arms'][a] for a in self.plugins}
            before, repeats = [], []
            for a in current['arms']:
                prepared = self.root / ('fixture-' + a + '.case')
                prepared.unlink(missing_ok=True)
                requested = routing_run.input_policy(self.spec, a)
                solver = 'auto' if requested is not None else self.spec['arms'][a].get('solver', a)
                if requested is not None:
                    routing.prepare(self.source, prepared, requested, self.cell['threads'])
                elif a == routing.COMPACT_IT:
                    routing.prepare(self.source, prepared, routing.COMPACT_IT, self.cell['threads'])
                else:
                    routing_run.cmgschur.prepare(self.source, prepared, solver, self.cell['threads'])
                raw = result(routing_run.EXPLICIT.get(solver, solver))
                for n in range(2):
                    raw['matrices']['RSOLVER']['bits'][n * 16 + 12] = bits(1. if solver == 'lsmr' else 0.)
                    raw['matrices']['RSOLVER']['bits'][n * 16 + 13] = bits(3. if solver == 'lsmr' else 0.)
                signature = routing_run.exact_view(raw, False)
                signature['columns_sha256'] = hashlib.sha256(self.data.tobytes()).hexdigest()
                r = {'cell': self.cell['id'], 'arm': a, 'status': 'passed',
                     'certificate_pass': True, 'science_pass': True,
                     'prepared_sha256': digest(prepared),
                     'science_sha256': hashlib.sha256(json.dumps(signature, sort_keys=True).encode()).hexdigest(),
                     'witness_sha256': h('fixture-witness'),
                     'resource_checks': {'iteration_caps': 'passed', 'memory_caps': 'passed', 'setup_caps': 'passed'}}
                before.append(r)
                repeats.append({**r, 'repetition': 0})
            outcomes = routing_run.prerequisite_outcomes(self.spec, list(current['arms']), before, repeats)
            current['arm_outcomes'] = outcomes
            proof = {k: current[k] for k in ['schema', 'status', 'manifest_sha256', 'cell_id',
                     'case_sha256', 'body_sha256', 'threads', 'arms', 'arm_outcomes', 'stage', 'selection_sha256']}
            proof.update(prerequisites=before, repeat_checks=repeats,
                         tool_sha256={p.name: v for p, v in routing_run.tool_identities().items()})
            sealed = self.root / 'fixture-prerequisite.json'
            sealed.write_text(json.dumps(routing.seal(proof)))
            current['scoped_replay'] = {'path': sealed.name, 'sha256': digest(sealed)}
        self.witness.write_text(json.dumps(current))
        with patch.object(routing_run, 'inspect_case', return_value=self.meta), \
                patch.object(routing_run.cmgschur, 'oracle_check'), \
                patch.object(routing_run.cmgschur, 'run_process', side_effect=native):
            output = routing_run.run(self.spec, stage, previous, self.harness, self.plugins,
                        {self.cell['id']: self.source}, {self.cell['id']: self.witness} if witnesses is None else witnesses,
                        self.root / output_name, ablation_only=ablation_only, prerequisites_only=prerequisites_only)
        return output, calls

    def test_successful_producer_is_directly_consumed_by_selector(self):
        output, calls = self.run_panel()
        self.assertEqual(output['status'], 'complete')
        self.assertEqual(set(output['summary'][0]['arms']), set(self.spec['arms']))
        self.assertEqual(output['summary'][0]['arms']['h4-k0']['over_baseline']['median_ratio'], 1.)
        self.assertEqual(len(output['prerequisites']), 6)
        self.assertTrue(all('solver_rows' not in row for row in output['prerequisites']))
        self.assertEqual(len(output['records']), 24)
        self.assertEqual(sum(traced for _, traced in calls), 6)
        self.assertEqual(sum(not traced for _, traced in calls), 24)
        self.assertTrue(all(r['certificate_pass'] and r['science_pass'] for r in output['records']))
        self.assertTrue(all(r['action_resources'][0]['setup_work_used'] == 12 for r in output['records'] if r['arm'].startswith('h')))
        decision = routing.evaluate(self.spec, 'A', output)
        self.assertEqual(decision['shortlist'], ['baseline', 'h32-k0', 'h16-k0', 'h8-k0'])
        self.assertEqual(len(set(r['prepared_sha256'] for r in output['records'])), 6)
        self.assertEqual(list((self.root / 'output').glob('*.case')), [])
        self.assertEqual(list((self.root / 'output').glob('*.columns')), [])
        self.assertGreater(len({tuple(r['arm'] for r in output['records'] if r['repetition'] == n)
                                for n in range(3)}), 1)

    def test_cmg_worker_guard_defers_wrappers_blocks_both_orders_and_preserves_repeat_checks(self):
        self.use_it_comparator('cmg')
        template = copy.deepcopy(self.cell)
        for index, (change, reverse) in enumerate([('warning', False), ('warning', True),
                ('fallback', False), ('fallback', True), ('repeat-warning', False), ('matching-warning', False),
                ('pending-worker', False)]):
            cells = [{**template, 'id': f'worker-{worker}', 'threads': worker} for worker in [1, 4]]
            if change == 'pending-worker':
                cells.append({**template, 'id': 'worker-8', 'threads': 8})
            if reverse:
                cells.reverse()
            self.spec['panels']['A'] = cells
            output_dir = self.root / f'cross-worker-{index}'
            witnesses = {}
            for cell in cells:
                witness = self.root / f'witness-{index}-{cell["id"]}.json'
                witness.write_text(json.dumps({**self.evidence, 'manifest_sha256': routing.identity(self.spec),
                    'cell_id': cell['id'], 'threads': cell['threads'], 'arms': self.spec['arms'],
                    'status': 'pending' if cell['threads'] == 8 else 'ready'}))
                witnesses[cell['id']] = witness
            def native(command, log, timeout):
                self.assertFalse(list(output_dir.glob('witness-*.json')))
                self.assertFalse(list(output_dir.glob('proof-*.json')))
                with Path(command[-2]).open('rb') as stream:
                    _, args = case_header(stream)
                fields = dict(a.decode().split('=', 1) for a in args[1:])
                traced = 'FEREG_SOLVER_TRACE=1' in command
                raw = ExplicitComparatorScope.raw(fields['solver'])
                worker = int(fields['threads'])
                self.assertNotEqual(worker, 8)
                if fields['solver'] == 'cmg':
                    if change == 'matching-warning' or (worker == 4 and (change == 'warning' or change == 'repeat-warning' and not traced)):
                        raw['matrices']['RSOLVER']['bits'][11] = bits(1.)
                    if worker == 4 and change == 'fallback':
                        for column, value in {1: 4., 10: 1., 11: 1.}.items():
                            raw['matrices']['RSOLVER']['bits'][16 + column] = bits(value)
                target = Path(command[-1])
                target.write_text(json.dumps({**raw, 'columns': []}))
                self.data.tofile(target.with_suffix('.json.columns'))
                trace = TRACE.replace('preset=routing_h4_k0', 'preset=' + fields['inject']) if traced and 'inject' in fields else ''
                log.write_text('NATIVE_SECONDS=0.1\n' + trace)
                return {'wall_s': .1, 'maxrss_bytes': 10}
            with self.subTest(change=change, reverse=reverse), \
                    patch.object(routing_run, 'inspect_case', return_value=self.meta), \
                    patch.object(routing_run.cmgschur, 'oracle_check'), \
                    patch.object(routing_run.cmgschur, 'run_process', side_effect=native):
                output = routing_run.run(self.spec, 'A', None, self.harness, self.plugins,
                    {c['id']: self.source for c in cells}, witnesses, output_dir, prerequisites_only=True)
            cmg = [row for row in [*output['prerequisites'], *output['records']] if row['arm'] == 'cmg']
            if change in {'warning', 'fallback'}:
                self.assertEqual(output['qualified_wrappers'], {})
                self.assertTrue(all(row['status'] == 'blocked' and row['prior_status'] == 'passed' for row in cmg))
                self.assertEqual(output['cmg_worker_checks'][0]['status'], 'blocked')
                self.assertTrue(all(row['certificate_pass'] and row['science_pass'] for row in cmg))
            elif change == 'repeat-warning':
                self.assertEqual(set(output['qualified_wrappers']), {'worker-1'})
                row = next(row for row in output['records'] if row['arm'] == 'cmg' and row['cell'] == 'worker-4')
                self.assertEqual(row['status'], 'blocked')
                self.assertIn('differs from traced prerequisite', row['error'])
            elif change == 'pending-worker':
                self.assertEqual(set(output['qualified_wrappers']), {'worker-1', 'worker-4'})
                check = output['cmg_worker_checks'][0]
                self.assertEqual(check['status'], 'not_compared')
                self.assertFalse(check['complete_coverage'])
                self.assertEqual(check['compared_worker_counts'], [1, 4])
                self.assertEqual(len(check['members']), 3)
                self.assertFalse(check['members'][-1]['complete'])
            else:
                self.assertEqual(set(output['qualified_wrappers']), {'worker-1', 'worker-4'})
                self.assertEqual(output['cmg_worker_checks'][0]['status'], 'passed')
                for cell in cells:
                    wrapper = output_dir / output['qualified_wrappers'][cell['id']]['path']
                    routing_run.evidence(wrapper, cell, self.spec, list(self.spec['arms']))

    def test_generic_it_worker_guard_publishes_only_matching_raw_diagnostics(self):
        self.use_it_comparator('it')
        self.meta['categorical_intercepts'] = False
        template = copy.deepcopy(self.cell)
        for index, (change, reverse) in enumerate([('matching', False), ('warning', False),
                                                  ('warning', True), ('repeat-warning', False)]):
            cells = [{**template, 'id': f'it-worker-{worker}', 'threads': worker} for worker in [1, 4]]
            if reverse:
                cells.reverse()
            self.spec['panels']['A'] = cells
            output_dir = self.root / f'it-cross-worker-{index}'
            witnesses = {}
            for cell in cells:
                witness = self.root / f'it-witness-{index}-{cell["id"]}.json'
                witness.write_text(json.dumps({**self.evidence, 'manifest_sha256': routing.identity(self.spec),
                    'cell_id': cell['id'], 'threads': cell['threads'], 'arms': self.spec['arms']}))
                witnesses[cell['id']] = witness
            def native(command, log, timeout):
                self.assertFalse(list(output_dir.glob('witness-*.json')))
                with Path(command[-2]).open('rb') as stream:
                    _, args = case_header(stream)
                fields = dict(a.decode().split('=', 1) for a in args[1:])
                traced = 'FEREG_SOLVER_TRACE=1' in command
                raw = ExplicitComparatorScope.raw(fields['solver'], 2, 10, 3)
                if fields['solver'] == 'it':
                    count = 1 if fields['threads'] == '4' and (change == 'warning' or
                        change == 'repeat-warning' and not traced) else 2
                    raw['matrices']['RSOLVER']['bits'][11] = bits(float(count))
                    raw['matrices']['RSOLVER']['bits'][27] = bits(1.)
                target = Path(command[-1])
                target.write_text(json.dumps({**raw, 'columns': []}))
                self.data.tofile(target.with_suffix('.json.columns'))
                trace = TRACE.replace('preset=routing_h4_k0', 'preset=' + fields['inject']) if traced and 'inject' in fields else ''
                log.write_text('NATIVE_SECONDS=0.1\n' + trace)
                return {'wall_s': .1, 'maxrss_bytes': 10}
            with self.subTest(change=change, reverse=reverse), \
                    patch.object(routing_run, 'inspect_case', return_value=self.meta), \
                    patch.object(routing_run.cmgschur, 'oracle_check'), \
                    patch.object(routing_run.cmgschur, 'run_process', side_effect=native):
                output = routing_run.run(self.spec, 'A', None, self.harness, self.plugins,
                    {c['id']: self.source for c in cells}, witnesses, output_dir, prerequisites_only=True)
            checks = output['generic_it_worker_checks']
            self.assertEqual(len(checks), 1)
            if change == 'matching':
                self.assertEqual(checks[0]['status'], 'passed')
                self.assertEqual(set(output['qualified_wrappers']), {'it-worker-1', 'it-worker-4'})
            elif change == 'warning':
                self.assertEqual(checks[0]['status'], 'blocked')
                self.assertEqual(output['qualified_wrappers'], {})
                it_rows = [r for r in [*output['prerequisites'], *output['records']] if r['arm'] == 'it']
                self.assertTrue(all(r['status'] == 'blocked' and r['prior_status'] == 'passed' for r in it_rows))
            else:
                self.assertEqual(set(output['qualified_wrappers']), {'it-worker-1'})
                row = next(r for r in output['records'] if r['arm'] == 'it' and r['cell'] == 'it-worker-4')
                self.assertEqual(row['status'], 'blocked')
                self.assertIn('differs from traced prerequisite', row['error'])

    def test_failed_prerequisites_or_repeat_failures_remain_infinite(self):
        for failure in ['timeout', 'missing-trace', 'repeat']:
            with self.subTest(failure=failure):
                output, calls = self.run_panel(failure)
                self.assertEqual(output['status'], 'complete_with_failed_cells')
                self.assertEqual(len(output['records']), 24)
                self.assertEqual(output['summary'][0]['arms']['h4-k0']['runtime_identified'], False)
                self.assertTrue(all(r['status'] != 'passed' for r in output['records'] if r['arm'] == 'h4-k0'))
                selected = routing.evaluate(self.spec, 'A', output)
                self.assertNotIn('h4-k0', selected['shortlist'])
                if failure != 'repeat':
                    self.assertNotIn(('routing_h4_k0', False), calls)
                for path in (self.root / 'output').iterdir():
                    path.unlink()
                (self.root / 'output').rmdir()

    def test_minimal_ablation_manifest_needs_no_tournament_or_holdout(self):
        self.spec['purpose'] = 'ablation_only'
        self.spec['candidates'] = ['baseline']
        self.spec['panels'] = {'A': self.spec['panels']['A']}
        self.cell['coverage'] = ['test-only-two-fe']
        self.cell['comparators'] = ['direct']
        self.spec['arms'] = {'baseline': self.spec['arms']['baseline'],
                             'direct': {'sha256': digest(self.plugin), 'role': 'optional_comparator'}}
        self.plugins = {'baseline': self.baseline, 'direct': self.plugin}
        self.evidence.update(arms=copy.deepcopy(self.spec['arms']), manifest_sha256=routing.identity(self.spec))
        self.witness.write_text(json.dumps(self.evidence))
        output, calls = self.run_panel(ablation_only=True)
        self.assertEqual(output['status'], 'complete')
        self.assertEqual(len(output['records']), 8)
        self.assertEqual({r['arm'] for r in output['records']}, {'baseline', 'direct'})
        with self.assertRaisesRegex(ValueError, 'cannot select'):
            routing.evaluate(self.spec, 'A', output)

    def use_it_comparator(self, name='it'):
        self.spec['arms'][name] = self.spec['arms'].pop('lsmr')
        for cells in self.spec['panels'].values():
            for cell in cells:
                cell['comparators'] = [name]
        self.plugins[name] = self.plugins.pop('lsmr')
        self.evidence.update(arms=copy.deepcopy(self.spec['arms']), manifest_sha256=routing.identity(self.spec))
        self.witness.write_text(json.dumps(self.evidence))

    def test_optional_native_exhaustion_keeps_candidate_proof_and_is_not_retried_in_timing(self):
        self.use_it_comparator()
        proof, calls = self.run_panel('optional-exhaustion', prerequisites_only=True)
        self.assertEqual(sum(arm == 'it' for arm, _ in calls), 1)
        link = proof['qualified_wrappers'][self.cell['id']]
        self.witness = self.root / 'output' / link['path']
        value, bound = routing_run.evidence(self.witness, self.cell, self.spec, list(self.spec['arms']))
        self.assertEqual(value['arm_outcomes']['it']['status'], 'unavailable')
        first = next(r for r in proof['prerequisites'] if r['arm'] == 'it')
        self.assertEqual(first['failure']['return_code'], 459)
        self.assertTrue(first['failure_artifacts'])
        self.assertEqual(first['maxrss_bytes'], 10)
        timed, calls = self.run_panel(existing_proof=True, output_name='timed')
        self.assertFalse(any(arm == 'it' for arm, _ in calls))
        decision = routing.evaluate(self.spec, 'A', timed)
        self.assertTrue(decision['shortlist'])
        self.assertTrue(all(c['comparisons'][1]['status'] == 'availability_dominance' for c in decision['ranking']))

    def test_failed_columns_are_retained_with_unchanged_outcome_and_checked_hash(self):
        self.use_it_comparator()
        for failure, status in [('optional-exhaustion', 'unavailable'), ('optional-wrong-science', 'blocked')]:
            with self.subTest(failure=failure):
                output, calls = self.run_panel(failure, prerequisites_only=True, output_name=failure)
                row = next(r for r in output['prerequisites'] if r['arm'] == 'it')
                self.assertEqual(row['status'], status)
                self.assertEqual(sum(arm == 'it' for arm, _ in calls), 1)
                retained = self.root / failure / row['failure_artifacts']['columns']['path']
                self.assertEqual(retained.read_bytes(), self.data.tobytes())
                self.assertEqual(digest(retained), row['failure_artifacts']['columns']['sha256'])
                self.assertFalse((self.root / failure / 'current.json.columns').exists())
                self.assertEqual(bool(output['qualified_wrappers']), status == 'unavailable')
                if status == 'unavailable':
                    wrapper = self.root / failure / output['qualified_wrappers'][self.cell['id']]['path']
                    routing_run.evidence(wrapper, self.cell, self.spec, list(self.spec['arms']))
                    retained.write_bytes(b'tampered failure output')
                    with self.assertRaisesRegex(ValueError, 'retained native failure artifact changed'):
                        routing_run.evidence(wrapper, self.cell, self.spec, list(self.spec['arms']))

    def test_optional_unknown_native_error_or_wrong_science_blocks_a_wrapper(self):
        self.use_it_comparator()
        for failure in ['optional-unknown', 'optional-wrong-science', 'optional-resource-overrun']:
            with self.subTest(failure=failure):
                out, calls = self.run_panel(failure, prerequisites_only=True, output_name=failure)
                self.assertEqual(out['qualified_wrappers'], {})
                row = next(r for r in out['prerequisites'] if r['arm'] == 'it')
                self.assertEqual(row['status'], 'blocked')
                self.assertEqual(sum(arm == 'it' for arm, _ in calls), 1)

    def test_cmgschur_nonconvergence_requires_exact_code_label_message_and_family(self):
        self.use_it_comparator('cmgschur')
        for kind in ['nonconvergence', 'wrong-code', 'wrong-label', 'unknown']:
            with self.subTest(kind=kind):
                out, calls = self.run_panel('optional-cmg-' + kind, prerequisites_only=True, output_name=kind)
                row = next(r for r in out['prerequisites'] if r['arm'] == 'cmgschur')
                self.assertEqual(sum(arm == 'cmgschur' for arm, _ in calls), 1)
                self.assertEqual(row['status'], 'unavailable' if kind == 'nonconvergence' else 'blocked')
                self.assertEqual(bool(out['qualified_wrappers']), kind == 'nonconvergence')
                if kind == 'nonconvergence':
                    self.assertEqual(row['failure']['kind'], 'certification_nonconvergence')
                    altered = copy.deepcopy(self.spec)
                    altered['arms']['cmgschur']['solver'] = 'it'
                    self.assertFalse(routing.terminal_failure(row, altered))

    def test_successful_prerequisite_fingerprint_must_bridge_to_timing(self):
        proof, _ = self.run_panel(prerequisites_only=True)
        self.witness = self.root / 'output' / proof['qualified_wrappers'][self.cell['id']]['path']
        timed, _ = self.run_panel('cohort-change', existing_proof=True, output_name='timed')
        self.assertTrue(any('checked prerequisite cohort' in r.get('error', '') for r in timed['prerequisites']))
        self.assertEqual(routing.evaluate(self.spec, 'A', timed)['shortlist'], [])

    def test_ready_dependencies_are_checked_before_during_and_after_native_collection(self):
        self.bound_dependency = self.root / 'frozen-local-oracle'
        self.bound_dependency.write_text('independent fixed target')
        original = self.bound_dependency.read_text()
        binding = {'path': self.bound_dependency.name, 'sha256': digest(self.bound_dependency)}
        self.evidence['frozen_dependencies'] = [binding]
        self.witness.write_text(json.dumps(self.evidence))
        self.bound_dependency.write_text('changed before collection')
        blocked, calls = self.run_panel(prerequisites_only=True, output_name='before')
        self.assertEqual(calls, [])
        self.assertEqual(blocked['qualified_wrappers'], {})
        self.bound_dependency.write_text(original)
        with patch.object(routing_run, 'tool_identities', return_value={self.bound_dependency.resolve(): h('identity drift')}):
            with self.assertRaisesRegex(ValueError, 'after frozen dependency validation'):
                self.run_panel(prerequisites_only=True, output_name='snapshot-conflict')
        self.assertFalse((self.root / 'snapshot-conflict').exists())
        with self.assertRaisesRegex(ValueError, 'changed'):
            self.run_panel('frozen-dependency-mutation', prerequisites_only=True, output_name='during')
        self.assertFalse(list((self.root / 'during').glob('witness-*.json')))
        self.bound_dependency.write_text(original)
        passed, _ = self.run_panel(prerequisites_only=True, output_name='passed')
        wrapper = self.root / 'passed' / passed['qualified_wrappers'][self.cell['id']]['path']
        value, _ = routing_run.evidence(wrapper, self.cell, self.spec, list(self.spec['arms']))
        self.assertEqual(value['frozen_dependencies'], [{**binding, 'path': str(self.bound_dependency.resolve())}])
        self.bound_dependency.write_text('changed after collection')
        with self.assertRaisesRegex(ValueError, 'frozen dependency hash mismatch'):
            routing_run.evidence(wrapper, self.cell, self.spec, list(self.spec['arms']))

    def historical_source_proof(self, suffix='historical'):
        logical = self.root / f'{suffix}-checker.py'
        logical.write_text('original reviewed source bytes\n')
        snapshot = self.root / f'{suffix}-snapshot.py'
        snapshot.write_bytes(logical.read_bytes())
        self.evidence['frozen_dependencies'] = [{'path': logical.name, 'sha256': digest(logical)}]
        self.witness.write_text(json.dumps(self.evidence))
        output, _ = self.run_panel(prerequisites_only=True, output_name=suffix)
        wrapper = self.root / suffix / output['qualified_wrappers'][self.cell['id']]['path']
        value = json.loads(wrapper.read_text())
        proof_path = wrapper.parent / value['scoped_replay']['path']
        proof = routing.unseal(json.loads(proof_path.read_text()))
        old_tools = {**proof['tool_sha256'], 'routing_run.py': digest(logical)}
        proof['tool_sha256'] = old_tools
        proof_path.write_text(json.dumps(routing.seal(proof)))
        for name in ['scoped_replay', 'resource_contract']:
            value[name]['sha256'] = digest(proof_path)
        wrapper.write_text(json.dumps(value))
        logical.write_text('changed live source bytes\n')
        return wrapper, logical.resolve(), snapshot.resolve(), old_tools, proof_path

    def test_historical_source_needs_explicit_old_tools_and_checked_snapshot_resolution(self):
        wrapper, logical, snapshot, old_tools, proof = self.historical_source_proof()
        original_proof = proof.read_bytes()
        calls = []
        def resolve(bound):
            calls.append(bound)
            self.assertEqual(bound, {'path': str(logical), 'sha256': digest(snapshot)})
            return snapshot
        arms = list(self.spec['arms'])
        with self.assertRaisesRegex(ValueError, 'frozen dependency hash mismatch'):
            routing_run.evidence(wrapper, self.cell, self.spec, arms)
        with self.assertRaisesRegex(ValueError, 'frozen dependency hash mismatch'):
            routing_run.evidence(wrapper, self.cell, self.spec, arms, expected_tools=old_tools)
        with self.assertRaisesRegex(ValueError, 'different producer or checkers'):
            routing_run.evidence(wrapper, self.cell, self.spec, arms, dependency_resolver=resolve)
        value, bound = routing_run.evidence(wrapper, self.cell, self.spec, arms,
            expected_tools=old_tools, dependency_resolver=resolve)
        self.assertEqual(value['frozen_dependencies'],
                         [{'path': str(logical), 'sha256': digest(snapshot)}])
        self.assertIn(snapshot, bound)
        self.assertNotIn(logical, bound)
        self.assertEqual(len(calls), 2)
        self.assertEqual(proof.read_bytes(), original_proof)
        self.assertEqual(routing.unseal(json.loads(proof.read_text()))['tool_sha256'], old_tools)

    def test_historical_source_snapshot_still_requires_exact_original_bytes(self):
        wrapper, logical, snapshot, old_tools, _ = self.historical_source_proof()
        snapshot.write_text('forged archived source\n')
        with self.assertRaisesRegex(ValueError, 'frozen dependency hash mismatch'):
            routing_run.evidence(wrapper, self.cell, self.spec, list(self.spec['arms']),
                expected_tools=old_tools, dependency_resolver=lambda _: snapshot)

    def test_historical_source_resolver_cannot_alias_independent_truth_or_replay_proofs(self):
        for name in ['independent_witness', 'scoped_replay', 'resource_contract']:
            with self.subTest(reference=name):
                wrapper, logical, snapshot, old_tools, _ = self.historical_source_proof(name)
                value = json.loads(wrapper.read_text())
                original = (wrapper.parent / value[name]['path']).resolve()
                # Give the selected reference its own path, so the named guard
                # is exercised even when replay and resource share one proof.
                current = self.root / f'{name}-live.json'
                current.write_bytes(original.read_bytes())
                preserved = self.root / f'{name}-preserved.json'
                preserved.write_bytes(current.read_bytes())
                value[name] = {'path': str(current), 'sha256': digest(current)}
                wrapper.write_text(json.dumps(value))
                current.write_text('changed independent evidence or proof\n')
                seen = []
                def resolve(bound):
                    seen.append(bound)
                    return snapshot if bound['path'] == str(logical) else preserved
                with self.assertRaisesRegex(ValueError, name + ': evidence file hash mismatch'):
                    routing_run.evidence(wrapper, self.cell, self.spec, list(self.spec['arms']),
                        expected_tools=old_tools, dependency_resolver=resolve)
                self.assertEqual(seen, [])

    def test_b_and_c_proofs_require_their_frozen_selection_and_own_case(self):
        for stage in ['B', 'C']:
            with self.subTest(stage=stage):
                source = self.root / (stage + '.case')
                source.write_bytes(case_bytes() + stage.encode())
                self.spec['panels'][stage][0]['case_sha256'] = digest(source)
                a = routing.evaluate(self.spec, 'A', records(self.spec, 'A'))
                previous = a if stage == 'B' else routing.evaluate(self.spec, 'B', records(self.spec, 'B', a), a)
                selected = routing.required_candidates(self.spec, stage, previous)
                self.cell = self.spec['panels'][stage][0]
                self.source = source
                self.meta['body_sha256'] = self.cell['body_sha256']
                self.plugins = {a: self.baseline if a == 'baseline' else self.plugin
                                for a in set(selected + self.spec['protections'] + self.cell['comparators'])}
                self.evidence.update(case_sha256=digest(source), body_sha256=self.cell['body_sha256'],
                                     cell_id=self.cell['id'], manifest_sha256=routing.identity(self.spec),
                                     arms=copy.deepcopy(self.spec['arms']))
                self.witness.write_text(json.dumps(self.evidence))
                out, _ = self.run_panel(prerequisites_only=True, stage=stage, previous=previous, output_name=stage)
                self.assertEqual(out['status'], 'complete')
                link = out['qualified_wrappers'][self.cell['id']]
                witness = self.root / stage / link['path']
                value, _ = routing_run.evidence(witness, self.cell, self.spec, list(self.plugins),
                                                stage=stage, selection_sha256=routing.identity(previous))
                self.assertEqual(value['stage'], stage)
                self.assertEqual(value['selection_sha256'], routing.identity(previous))
                with self.assertRaisesRegex(ValueError, 'frozen stage selection'):
                    routing_run.evidence(witness, self.cell, self.spec, list(self.plugins), stage=stage)

    def test_proof_rejects_changed_producer_extra_rows_or_changed_selection(self):
        proof, _ = self.run_panel(prerequisites_only=True)
        witness = self.root / 'output' / proof['qualified_wrappers'][self.cell['id']]['path']
        wrapper = json.loads(witness.read_text())
        proof_path = witness.parent / wrapper['scoped_replay']['path']
        original = routing.unseal(json.loads(proof_path.read_text()))
        for change in ['producer', 'extra', 'cell', 'selection']:
            value = copy.deepcopy(original)
            if change == 'producer':
                value['tool_sha256']['routing_run.py'] = h('changed')
            elif change == 'extra':
                value['repeat_checks'].append(copy.deepcopy(value['repeat_checks'][0]))
            elif change == 'cell':
                value['repeat_checks'][0]['cell'] = 'wrong cell'
            else:
                value['selection_sha256'] = h('not frozen selection')
            proof_path.write_text(json.dumps(routing.seal(value)))
            edited = copy.deepcopy(wrapper)
            for field in ['scoped_replay', 'resource_contract']:
                edited[field]['sha256'] = digest(proof_path)
            witness.write_text(json.dumps(edited))
            with self.subTest(change=change), self.assertRaises(ValueError):
                routing_run.evidence(witness, self.cell, self.spec, list(self.spec['arms']))

    def configure_experiment(self, spec, terms=3):
        self.spec = spec
        self.cell = self.spec['panels']['A'][0]
        self.cell['case_sha256'] = digest(self.source)
        self.meta.update(body_sha256=self.cell['body_sha256'], absorbed_terms=terms, categorical_intercepts=True)
        self.evidence.update(case_sha256=self.cell['case_sha256'], body_sha256=self.cell['body_sha256'],
                             manifest_sha256=routing.identity(spec), cell_id=self.cell['id'], threads=self.cell['threads'],
                             arms=copy.deepcopy(spec['arms']))
        self.witness.write_text(json.dumps(self.evidence))
        self.plugins = {a: self.baseline if a == 'baseline' else self.plugin for a in spec['arms']}

    def test_isolated_prefix_producer_keeps_exact_requested_prefix_and_no_hk_product(self):
        self.configure_experiment(prefix_manifest())
        out, calls = self.run_panel(prerequisites_only=True)
        self.assertEqual(out['status'], 'complete')
        self.assertEqual(out['experiment_kind'], 'prefix')
        expected = {'auto', 'lsmr', *(routing.policy(a)['injection'] for a in routing.PREFIXES)}
        self.assertEqual({a for a, _ in calls}, expected)
        link = out['qualified_wrappers'][self.cell['id']]
        self.witness = self.root / 'output' / link['path']
        timed, _ = self.run_panel(existing_proof=True, output_name='timed')
        self.assertEqual(routing.evaluate(self.spec, 'A', timed)['shortlist'],
                         ['baseline', 'prefix-none', 'prefix-map4', 'prefix-it4'])

    def test_portfolio_producer_uses_distinct_strict_arm_without_the_prefix_grid(self):
        self.configure_experiment(portfolio_manifest())
        out, calls = self.run_panel(prerequisites_only=True)
        self.assertEqual(out['status'], 'complete')
        self.assertEqual(out['experiment_kind'], 'portfolio')
        self.assertEqual({a for a, _ in calls}, {'auto', 'lsmr', 'routing_portfolio_v1'})
        link = out['qualified_wrappers'][self.cell['id']]
        self.witness = self.root / 'output' / link['path']
        timed, _ = self.run_panel(existing_proof=True, output_name='timed')
        self.assertEqual(routing.evaluate(self.spec, 'A', timed)['shortlist'],
                         ['baseline', routing.PORTFOLIO_V1])

    def test_actual_portfolio_producer_rows_feed_parallel_discrete_science(self):
        import routing_parallel as parallel
        self.configure_experiment(portfolio_manifest())
        output, _ = self.run_panel(prerequisites_only=True)
        self.assertEqual(output['status'], 'complete')
        # run_panel writes raw native JSON and uses the real cmgschur.read_run;
        # neither the records nor the diagnostic fields are fabricated here.
        outcomes = routing_run.prerequisite_outcomes(self.spec, list(self.spec['arms']),
            output['prerequisites'], output['records'])
        keyed = {(self.cell['id'], arm): outcome for arm, outcome in outcomes.items()}
        self.assertTrue(routing.sha(parallel._discrete_science(output, keyed)))
        for first in output['prerequisites']:
            repeat = next(r for r in output['records'] if r['arm'] == first['arm'])
            self.assertEqual(first['solver_rows'], repeat['solver_rows'])
            self.assertEqual(len(first['solver_rows']), 2)
            self.assertTrue(all(len(row) == 16 for row in first['solver_rows']))
            for row in first['solver_rows']:
                self.assertEqual(row[0], 3. if first['arm'] == 'lsmr' else 1.)
                self.assertEqual(row[1], 2. if first['arm'] == routing.PORTFOLIO_V1 else 3.)
        missing = copy.deepcopy(output)
        del missing['prerequisites'][0]['solver_rows']
        with self.assertRaisesRegex(ValueError, 'missing local solver diagnostics'):
            parallel._discrete_science(missing, keyed)
        changed = copy.deepcopy(output)
        changed['records'][0]['solver_rows'][0][14] += 1
        with self.assertRaisesRegex(ValueError, 'local discrete decisions changed'):
            parallel._discrete_science(changed, keyed)
        link = output['qualified_wrappers'][self.cell['id']]
        self.witness = self.root / 'output' / link['path']
        timed, _ = self.run_panel(existing_proof=True, output_name='portfolio-timed')
        self.assertEqual(timed['status'], 'complete')
        self.assertTrue(routing.sha(parallel._discrete_science(timed, keyed)))

    def test_portfolio_same_request_repeat_diagnostics_remain_exact(self):
        self.configure_experiment(portfolio_manifest())
        out, _ = self.run_panel(prerequisites_only=True, failure='portfolio-repeat')
        failed = [row for row in out['records'] if row['arm'] == routing.PORTFOLIO_V1]
        self.assertEqual(len(failed), 1)
        self.assertIn('differs from traced prerequisite', failed[0]['error'])
        self.assertNotEqual(failed[0]['status'], 'passed')
        # The wrapper preserves the failed candidate alongside a valid baseline;
        # it must never advertise this candidate as scientifically available.
        wrapper = json.loads((self.root / 'output' / out['qualified_wrappers'][self.cell['id']]['path']).read_text())
        self.assertEqual(wrapper['arm_outcomes'][routing.PORTFOLIO_V1]['status'], 'blocked')

    def test_equivalence_producer_signature_excludes_only_solver_timing(self):
        self.spec['purpose'] = 'activation_equivalence'
        current = json.loads(self.witness.read_text())
        current['manifest_sha256'] = routing.identity(self.spec)
        self.witness.write_text(json.dumps(current))
        out, _ = self.run_panel(prerequisites_only=True)
        self.assertEqual(out['status'], 'complete')
        for first in out['prerequisites']:
            repeat = next(row for row in out['records'] if row['arm'] == first['arm'])
            self.assertEqual(first['solver_signature_sha256'], repeat['solver_signature_sha256'])
            raw = result('lsmr' if first['arm'] == 'lsmr' else 'auto')
            for row in range(2):
                raw['matrices']['RSOLVER']['bits'][16 * row + 12] = bits(1. if first['arm'] == 'lsmr' else 0.)
                raw['matrices']['RSOLVER']['bits'][16 * row + 13] = bits(3. if first['arm'] == 'lsmr' else 0.)
            self.assertEqual(first['solver_signature_sha256'],
                routing.identity(routing_run.exact_view(raw, False)['matrices']['RSOLVER']))
            raw['matrices']['RSOLVER']['bits'][14] = bits(123.)
            self.assertNotEqual(first['solver_signature_sha256'],
                routing.identity(routing_run.exact_view(raw, False)['matrices']['RSOLVER']))

    def test_binary_overlap_producer_uses_the_frozen_serial_checkpoint(self):
        self.configure_experiment(overlap_manifest(), terms=2)
        out, calls = self.run_panel(prerequisites_only=True)
        self.assertEqual(out['status'], 'complete')
        self.assertEqual({a for a, _ in calls}, {'auto', 'lsmr', 'routing_h8_k0', 'routing_overlap_h8_k0'})
        self.assertEqual(routing_run.input_policy(self.spec, 'serial'), 'h8-k0')
        self.assertEqual(routing_run.input_policy(self.spec, 'overlap'), 'overlap-h8-k0')

    def test_public_it_and_compact_it_have_separate_prepared_requests_and_outcomes(self):
        self.use_it_comparator()
        self.spec['arms'][routing.COMPACT_IT] = {'sha256': digest(self.plugin), 'solver': 'it',
            'injection': 'it_coefficients', 'role': 'optional_comparator'}
        for cells in self.spec['panels'].values():
            cells[0].update(absorbed_terms=2, categorical_intercepts=True)
            cells[0]['comparators'].append(routing.COMPACT_IT)
        self.configure_experiment(self.spec, terms=2)
        out, calls = self.run_panel(prerequisites_only=True)
        self.assertEqual(out['status'], 'complete')
        self.assertIn(('it', True), calls)
        self.assertIn(('it_coefficients', True), calls)
        prepared = out['prepared_sha256']
        self.assertNotEqual(prepared[self.cell['id'] + '/it'], prepared[self.cell['id'] + '/it-coefficients'])
        link = out['qualified_wrappers'][self.cell['id']]
        wrapper = json.loads((self.root / 'output' / link['path']).read_text())
        self.assertEqual(wrapper['arm_outcomes']['it']['status'], 'passed')
        self.assertEqual(wrapper['arm_outcomes']['it-coefficients']['status'], 'passed')

    def test_declared_prefix_eligibility_must_match_the_model_before_any_native_call(self):
        self.configure_experiment(prefix_manifest())
        self.meta['categorical_intercepts'] = False
        with self.assertRaisesRegex(ValueError, 'eligibility differs'):
            self.run_panel(prerequisites_only=True)
        self.assertFalse((self.root / 'output').exists())

    def test_actual_resource_cap_and_no_fe_exception_are_not_inferred(self):
        for trace in ['', TRACE.replace('setup_work=12', 'setup_work=33'), TRACE.replace('certified=1', 'certified=0')]:
            with self.assertRaises(ValueError):
                routing_run.resource_diagnostics(trace, 'h4-k0', False, self.evidence)
        with self.assertRaises(ValueError):
            routing_run.resource_diagnostics('', 'h4-k0', True, self.evidence)
        allowed = routing_run.resource_diagnostics('', 'h4-k0', True, {**self.evidence, 'no_fe_exact_path': {}})
        self.assertEqual(allowed['resource_basis'], 'bound exact-no-FE witness')
        self.assertEqual(routing_run.resource_diagnostics('', None, False, self.evidence)['resource_checks']['setup_caps'],
                         'not_applicable')

    def test_inherited_instrumentation_and_route_environment_are_cleared(self):
        with patch.dict(os.environ, {'FEREG_CMG_ORDINARY': '1', 'FEREG_PHASE_TIMING': '1',
                                     'FEREG_MEMORY_LIMIT_BYTES': '1'}):
            output, _ = self.run_panel()
        self.assertEqual(output['status'], 'complete')
        self.assertEqual(output['cleared_environment'], list(routing_run.CLEARED_ENV))
        command = routing_run.native_command(self.harness, self.plugin, self.source, self.root / 'out',
                                            {**self.cell, 'memory_limit_bytes': 1234}, False)
        self.assertIn('FEREG_MEMORY_LIMIT_BYTES=1234', command)

    def test_actual_preset_and_handoff_parameters_must_match(self):
        with self.assertRaisesRegex(ValueError, 'requested preset'):
            routing_run.resource_diagnostics(TRACE, 'h8-k0', False, self.evidence)
        wrong = TRACE + 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=32|samples=2\n'
        with self.assertRaisesRegex(ValueError, 'does not match'):
            routing_run.resource_diagnostics(wrong, 'h4-k0', False, self.evidence)

    def test_portfolio_resources_validate_nested_two_fe_policy_and_all_caps(self):
        action = TRACE.replace('routing_h4_k0', 'routing_portfolio_v1')
        route = 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1\n'
        for log in [action, action + route]:
            checked = routing_run.resource_diagnostics(log, routing.PORTFOLIO_V1, False, self.evidence)
            self.assertEqual(checked['resource_checks'], {'iteration_caps': 'passed',
                'memory_caps': 'passed', 'setup_caps': 'passed'})
            self.assertEqual(checked['action_resources'][0]['preset'], 'routing_portfolio_v1')
        for invalid in ['', route, action + route.replace('checkpoint=16', 'checkpoint=8'),
                        action + route.replace('samples=1', 'samples=0'),
                        action.replace('setup_work=12', 'setup_work=33'),
                        action.replace('iterations=5', 'iterations=101'),
                        action.replace('memory_bytes=64', 'memory_bytes=129'),
                        action.replace('certified=1', 'certified=0'), TRACE]:
            with self.subTest(log=invalid), self.assertRaises(ValueError):
                routing_run.resource_diagnostics(invalid, routing.PORTFOLIO_V1, False, self.evidence)

    def test_evidence_worker_and_policy_scope_cannot_be_reused(self):
        for field, value in [('threads', 4), ('cell_id', 'B-cell'), ('manifest_sha256', h('other')),
                             ('arms', {**self.evidence['arms'], 'h4-k0': self.evidence['arms']['h8-k0']})]:
            original = copy.deepcopy(self.evidence)
            self.witness.write_text(json.dumps({**original, field: value}))
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'does not bind'):
                self.run_panel()
        self.assertFalse((self.root / 'output').exists())

    def test_incumbent_keeps_declared_preset_and_ablations_never_promote(self):
        self.spec['arms']['incumbent'] = {'role': 'required_reference', 'sha256': h('candidate'), 'policy': routing.policy('h16-k1')}
        self.spec['protections'].append('incumbent')
        self.assertEqual(routing_run.input_policy(self.spec, 'incumbent'), 'h16-k1')
        self.spec['arms']['incumbent']['policy'] = None
        self.assertEqual(routing_run.input_policy(self.spec, 'incumbent'), 'baseline')
        self.spec['protections'].remove('incumbent')
        del self.spec['arms']['incumbent']
        self.cell['comparators'] = list(routing.ABLATIONS)
        for arm in routing.ABLATIONS:
            self.spec['arms'][arm] = {'sha256': digest(self.plugin), 'role': 'optional_comparator'}
        self.plugins = {a: self.baseline if a == 'baseline' else self.plugin for a in ['baseline', *routing.ABLATIONS]}
        self.evidence.update(arms=copy.deepcopy(self.spec['arms']), manifest_sha256=routing.identity(self.spec))
        self.witness.write_text(json.dumps(self.evidence))
        output, calls = self.run_panel(ablation_only=True)
        self.assertEqual(output['purpose'], 'ablation_only')
        self.assertEqual(output['status'], 'complete')
        self.assertEqual({r['arm'] for r in output['records']}, {'baseline', *routing.ABLATIONS})
        self.assertFalse(any('routing_h' in arm for arm, _ in calls))
        with self.assertRaisesRegex(ValueError, 'cannot select'):
            routing.evaluate(self.spec, 'A', output)

    def test_copied_independent_replay_reference_is_checked_on_native_results(self):
        columns = self.root / 'oracle.columns'
        self.data.tofile(columns)
        expected = {'schema': 1, 'return_code': 0, 'poisson': False, 'rows': [0, 1],
                    'beta': [1.], 'covariance': [[1.]], 'sample': [1., 1.],
                    'residual': [.2, -.2], 'fitted': [.5, .2], 'outputs': self.meta['outputs'], 'saved': []}
        meta = {k: v for k, v in self.meta.items() if k != 'oracle'}
        meta['replay_reference'] = expected
        routing_run.check_oracle(result(), columns, meta)
        expected['residual'][0] = .3
        with self.assertRaisesRegex(ValueError, 'independent replay oracle'):
            routing_run.check_oracle(result(), columns, meta)

    def test_witness_identity_and_artifact_alias_are_checked_before_any_native_call(self):
        self.evidence['independent_witness']['sha256'] = h('not the witness')
        self.witness.write_text(json.dumps(self.evidence))
        with self.assertRaisesRegex(ValueError, 'file hash mismatch'):
            self.run_panel()
        self.plugins['baseline'] = self.plugin
        with self.assertRaisesRegex(ValueError, 'artifact does not match'):
            self.run_panel()
        self.assertFalse((self.root / 'output').exists())

    def test_failed_producer_cannot_be_selected_even_with_complete_rows(self):
        output, _ = self.run_panel()
        output['status'] = 'failed'
        with self.assertRaisesRegex(ValueError, 'producer did not complete'):
            routing.evaluate(self.spec, 'A', output)

    def test_prerequisite_mode_generates_bound_proof_after_actual_checks_only(self):
        del self.evidence['scoped_replay']
        self.witness.write_text(json.dumps(self.evidence))
        output, calls = self.run_panel(prerequisites_only=True)
        self.assertEqual(output['status'], 'complete')
        self.assertEqual(output['purpose'], 'scientific_prerequisites')
        self.assertEqual(len(calls), 12)
        self.assertEqual(len(output['records']), 6)
        self.assertEqual(output['summary'], [])
        self.assertTrue(all('seconds' not in r and 'native_s' not in r for r in output['records']))
        link = output['qualified_wrappers'][self.cell['id']]
        path = self.root / 'output' / link['path']
        self.assertEqual(digest(path), link['sha256'])
        value, bound = routing_run.evidence(path, self.cell, self.spec, list(self.spec['arms']))
        self.assertEqual(value['status'], 'checked')
        self.assertEqual(len(bound), 3)
        with self.assertRaisesRegex(ValueError, 'not timing observations'):
            routing.evaluate(self.spec, 'A', output)

    def test_prerequisites_leave_missing_independent_truth_pending(self):
        self.meta.pop('oracle')
        del self.evidence['independent_witness']
        self.witness.write_text(json.dumps(self.evidence))
        output, calls = self.run_panel(prerequisites_only=True)
        self.assertEqual(calls, [])
        self.assertEqual(output['status'], 'complete_with_failed_cells')
        self.assertEqual(len(output['records']), 6)
        self.assertEqual(output['qualified_wrappers'], {})
        self.assertTrue(all(r['status'] == 'blocked' for r in output['prerequisites']))

    def test_failed_repeat_cannot_create_a_prerequisite_wrapper(self):
        output, calls = self.run_panel('repeat', prerequisites_only=True)
        self.assertIn(self.cell['id'], output['qualified_wrappers'])
        link = output['qualified_wrappers'][self.cell['id']]
        checked = json.loads((self.root / 'output' / link['path']).read_text())
        self.assertEqual(checked['arm_outcomes']['h4-k0']['status'], 'blocked')
        self.assertEqual(output['status'], 'complete_with_failed_cells')

    def test_prerequisite_proof_requires_exact_unaffected_science(self):
        output, _ = self.run_panel('dof', prerequisites_only=True)
        self.assertIn(self.cell['id'], output['qualified_wrappers'])
        errors = [row.get('error', '') for row in output['prerequisites']]
        self.assertTrue(any('unaffected field' in error for error in errors))

    def test_default_route_engine_ids_and_tolerance_remain_exact(self):
        for failure in ['tolerance', 'used-engine']:
            output, _ = self.run_panel(failure, prerequisites_only=True)
            self.assertIn(self.cell['id'], output['qualified_wrappers'])
            self.assertEqual(output['status'], 'complete_with_failed_cells')
            for path in (self.root / 'output').iterdir():
                path.unlink()
            (self.root / 'output').rmdir()

    def test_artifact_mutation_cannot_leave_a_consumable_passing_wrapper(self):
        with self.assertRaisesRegex(ValueError, 'frozen artifact'):
            self.run_panel('mutation', prerequisites_only=True)
        out = self.root / 'output'
        self.assertEqual(list(out.glob('witness-*.json')), [])
        self.assertEqual(list(out.glob('proof-*.json')), [])
        self.assertEqual(json.loads((out / 'records.json').read_text())['qualified_wrappers'], {})

    def test_prerequisite_mode_cannot_examine_the_holdout(self):
        with self.assertRaisesRegex(ValueError, 'preceding frozen selection'):
            routing_run.run(self.spec, 'C', None, self.harness, {}, {}, {}, self.root / 'output',
                            prerequisites_only=True)
        self.assertFalse((self.root / 'output').exists())

    def test_typed_independent_reference_is_rerun_for_every_native_result(self):
        reference_path = self.root / 'independent.json'
        reference_path.write_text(json.dumps({'kind': routing_run.routing_oracle.KIND}))
        arrays = self.root / 'independent.npz'
        arrays.write_bytes(b'bound array fixture')
        self.evidence['independent_witness'] = {'path': reference_path.name, 'sha256': digest(reference_path)}
        self.witness.write_text(json.dumps(self.evidence))
        reference = {'fixture': 'independent truth', '_arrays_path': arrays}
        with patch.object(routing_run.routing_oracle, 'load_reference',
                          return_value=(reference, {arrays: digest(arrays)})) as load, \
                patch.object(routing_run.routing_oracle, 'check_reference') as check:
            output, calls = self.run_panel(prerequisites_only=True)
        load.assert_called_once_with(reference_path.resolve(), self.cell['case_sha256'], self.cell['body_sha256'])
        self.assertEqual(check.call_count, len(calls))
        self.assertEqual(len(calls), 12)
        link = output['qualified_wrappers'][self.cell['id']]
        wrapper = json.loads((self.root / 'output' / link['path']).read_text())
        self.assertEqual(wrapper['independent_witness']['path'], str(reference_path.resolve()))
        self.assertEqual(output['status'], 'complete')


if __name__ == '__main__':
    unittest.main()
