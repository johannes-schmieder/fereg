"""Numerical scopes, independent witnesses and resource certificates for qualification."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_science as science
from test_cmgschur import bits, result

TRACE = ('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_h4_k0|final=0|certified=1|iterations=5|native_limit=100|'
         'memory_bytes=64|memory_limit=128\n')


def base_raw(arm, mode=1, poolsize=1, capacity=1):
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


class PortfolioAutoScope(unittest.TestCase):
    META = {'categorical_intercepts': True, 'absorbed_terms': 3, 'rows': 2}

    @staticmethod
    def raw(route='lsmr', *, pilot=0, mode=1, warnings=None):
        raw = base_raw('auto', mode, 1, 3)
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
        reference = self.raw(mode=int(science.cmgschur.numbers(raw['matrices']['RRUNTIME'])[3])) if reference is None else reference
        with patch.object(science.cmgschur, 'scientific_comparison') as checked:
            science.compatibility(reference, raw, None, None,
                self.META if meta is None else meta, reader,
                selected_policy=routing.policy(routing.PORTFOLIO_V1) if policy is None else policy)
            checked.assert_called_once()

    def test_only_named_portfolio_leaves_follow_valid_actual_routes(self):
        for route, pilot in [('prefix', 0), ('prefix', 2), ('prefix', 4), ('lsmr', 4),
                             ('lsmr', 0), ('lsmr', 1), ('map-no-lsmr', 0),
                             ('map-fallback', 4), ('pc-fallback', 4)]:
            raw = self.raw(route, pilot=pilot)
            before = copy.deepcopy(raw)
            with self.subTest(route=route, pilot=pilot):
                self.compare(raw)
                scope = set(science.portfolio_scope(self.raw(), raw, self.META, 'auto',
                            routing.policy(routing.PORTFOLIO_V1)))
                old = set(science.replay_scope(raw, 'auto', self.META))
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
        with patch.object(science.cmgschur, 'scientific_comparison'), self.assertRaisesRegex(ValueError, 'unaffected field'):
            science.compatibility(self.raw(), raw, None, None, self.META, 'auto')
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
        self.assertEqual(science.portfolio_scope(raw, raw, meta, 'auto', selected),
                         science.replay_scope(raw, 'auto', meta))
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


    def test_two_fe_direct_changes_only_the_validated_family_leaf(self):
        meta = {**self.META, 'absorbed_terms': 2}
        before, after = self.raw('map-no-lsmr'), self.raw('prefix')
        for row in range(2):
            before['matrices']['RSOLVER']['bits'][16 * row + 1] = bits(4.)
        # Primary direct certification uses the requested, not private, target.
        after['matrices']['RSOLVER']['bits'][5] = bits(5e-9)
        self.compare(after, reference=before, meta=meta)
        scope = set(science.portfolio_scope(before, after, meta, 'auto', routing.policy(routing.PORTFOLIO_V1)))
        self.assertEqual(scope - set(science.replay_scope(after, 'auto', meta)),
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


class UnseededPortfolioScope(unittest.TestCase):
    META = PortfolioAutoScope.META
    raw = staticmethod(PortfolioAutoScope.raw)

    def compare(self, raw, **kwargs):
        return PortfolioAutoScope.compare(self, raw, policy=routing.policy(routing.PORTFOLIO_UNSEEDED), **kwargs)

    def test_zero_pilot_lsmr_and_source_supported_map_paths_keep_existing_certificates(self):
        for mode in (1, 2):
            for route in ('lsmr', 'pc-fallback', 'map-fallback', 'map-no-lsmr'):
                raw = self.raw(route, pilot=0, mode=mode)
                reference = self.raw('lsmr', pilot=4, mode=mode)
                with self.subTest(mode=mode, route=route):
                    self.compare(raw, reference=reference)
                    scope = set(science.portfolio_scope(reference, raw, self.META, 'auto',
                                routing.policy(routing.PORTFOLIO_UNSEEDED)))
                    self.assertEqual(scope - set(science.replay_scope(raw, 'auto', self.META)),
                        {f'/matrices/RSOLVER/bits/{14*r+c}' for r in range(2) for c in [1,8,9,10,11,12]})
        # A workspace decline before LSMR produces no attempted-report fallback
        # bit. Its certified original-input MAP can still take positive work.
        raw = self.raw('map-no-lsmr', pilot=0)
        self.assertEqual(science.cmgschur.numbers(raw['matrices']['RSOLVER'])[2], 10)
        self.compare(raw)
        raw = self.raw('map-fallback', pilot=0, mode=2)
        raw['matrices']['RSOLVER']['bits'][11] = bits(4.)
        self.compare(raw)

    def test_zero_pilot_alone_does_not_admit_forged_routes_or_loose_certificates(self):
        mutations = [(0, 3.), (1, 4.), (3, 3.), (10, 2.), (11, .5), (11, -1.),
                     (12, 0.), (12, 2.), (13, 0.), (14, 1.), (14, 4.), (15, .1),
                     (5, 2e-10), (2, 101.), (2, float('nan'))]
        for row in range(2):
            for column, value in mutations:
                raw = self.raw('lsmr', pilot=0)
                raw['matrices']['RSOLVER']['bits'][16*row+column] = bits(value)
                with self.subTest(row=row, column=column), self.assertRaises(ValueError): self.compare(raw)
        for route, column, value in [('map-fallback',10,0.), ('map-fallback',11,0.),
                                     ('pc-fallback',13,4.), ('map-no-lsmr',11,1.)]:
            for row in range(2):
                raw = self.raw(route, pilot=0)
                raw['matrices']['RSOLVER']['bits'][16*row+column] = bits(value)
                with self.subTest(route=route,row=row,column=column), self.assertRaises(ValueError): self.compare(raw)
        raw = self.raw(); raw['matrices']['RSOLVER']['bits'][21] = bits(2e-14)
        with self.assertRaisesRegex(ValueError,'private Auto target'): self.compare(raw)
        for key, value in [('return_code',1), ('return_code',True)]:
            raw = self.raw(); raw[key] = value
            with self.assertRaises(ValueError): self.compare(raw)
        for rows, columns in [(1,32), (2,15), (True,16)]:
            raw = self.raw(); raw['matrices']['RSOLVER'].update(rows=rows,columns=columns)
            with self.assertRaises(ValueError): self.compare(raw)

    def test_unseeded_two_fe_scope_and_direct_certificate_are_identical_to_v1(self):
        meta = {**self.META,'absorbed_terms':2}
        before, after = self.raw('map-no-lsmr'), self.raw('prefix')
        for row in range(2): before['matrices']['RSOLVER']['bits'][16*row+1] = bits(4.)
        after['matrices']['RSOLVER']['bits'][5] = bits(5e-9)
        for raw, reference in [(self.raw(), self.raw()), (after,before)]:
            self.assertEqual(science.portfolio_scope(reference,raw,meta,'auto',routing.policy(routing.PORTFOLIO_UNSEEDED)),
                             science.portfolio_scope(reference,raw,meta,'auto',routing.policy(routing.PORTFOLIO_V1)))
            self.compare(raw,reference=reference,meta=meta)
        for row, column, value in [(0,5,2e-8),(1,5,2e-14),(0,14,1.),(1,10,1.)]:
            changed = copy.deepcopy(after); changed['matrices']['RSOLVER']['bits'][16*row+column] = bits(value)
            with self.subTest(row=row,column=column), self.assertRaises(ValueError): self.compare(changed,reference=before,meta=meta)


    def test_exact_unseeded_preset_and_two_fe_resource_caps(self):
        action = TRACE.replace('routing_h4_k0','routing_portfolio_unseeded')
        route = 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1\n'
        for log in (action,action+route):
            checked = science.resource_diagnostics(log,routing.PORTFOLIO_UNSEEDED,False,{})
            self.assertEqual(checked['action_resources'][0]['preset'],'routing_portfolio_unseeded')
        for log in ('',TRACE,action.replace('certified=1','certified=0'),
                    action.replace('iterations=5','iterations=101'),
                    action.replace('memory_bytes=64','memory_bytes=129'),action+route.replace('samples=1','samples=0'),
                    action+route.replace('checkpoint=16','checkpoint=32')):
            with self.subTest(log=log),self.assertRaises(ValueError):
                science.resource_diagnostics(log,routing.PORTFOLIO_UNSEEDED,False,{})
        with self.assertRaises(ValueError):
            science.resource_diagnostics(TRACE.replace('routing_h4_k0','routing_portfolio_v1'),
                routing.PORTFOLIO_UNSEEDED,False,{},expected_preset='routing_portfolio_v1')


class RouteHierarchyTrace(unittest.TestCase):
    PREFIX = 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1|iterations=16|'
    METADATA = ('terminal=Direct|levels=2|hierarchy_vertices=64;9|hierarchy_nonzeros=448;41|repeats=3;0|'
                'terminal_factor=1|terminal_factor_dimension=8|terminal_factor_nonzeros=19|'
                'application=scalar|parallel_operators=0|batch_columns=1')
    INSTALLED = PREFIX + 'installed=1|refusal=None|' + METADATA + '\n'
    REFUSED = PREFIX + 'installed=0|refusal=Some(MemoryBudget)\n'

    def diagnostics(self, route):
        action = TRACE.replace('routing_h4_k0', 'routing_portfolio_unseeded')
        return science.resource_diagnostics(action + route, routing.PORTFOLIO_UNSEEDED, False, {})

    def test_installed_metadata_is_parsed_and_refusals_carry_none(self):
        hierarchy = self.diagnostics(self.INSTALLED)['route_hierarchy']
        self.assertEqual(hierarchy, [{'terminal': 'Direct', 'levels': 2, 'vertices': [64, 9],
            'nonzeros': [448, 41], 'repeats': [3, 0], 'terminal_factor': 1,
            'terminal_factor_dimension': 8, 'terminal_factor_nonzeros': 19,
            'application': 'scalar', 'parallel_operators': 0, 'batch_columns': 1}])
        iterative = self.INSTALLED.replace('terminal=Direct', 'terminal=StagnatedFill').replace(
            'terminal_factor=1|terminal_factor_dimension=8|terminal_factor_nonzeros=19',
            'terminal_factor=0|terminal_factor_dimension=0|terminal_factor_nonzeros=0')
        self.assertEqual(self.diagnostics(iterative)['route_hierarchy'][0]['terminal_factor'], 0)
        self.assertEqual(self.diagnostics(self.REFUSED)['route_hierarchy'], [None])
        self.assertEqual(self.diagnostics(self.REFUSED + self.INSTALLED)['route_hierarchy'][0], None)
        # Records without an installation flag predate the attestation.
        legacy = 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1\n'
        self.assertEqual(self.diagnostics(legacy)['route_hierarchy'], [None])

    def test_installed_without_metadata_differs_from_refusal_and_invalid_metadata_fails(self):
        bare = self.PREFIX + 'installed=1|refusal=None\n'
        with self.assertRaisesRegex(ValueError, 'lacks hierarchy metadata'):
            self.diagnostics(bare)
        partial = self.INSTALLED.replace('|batch_columns=1', '')
        with self.assertRaisesRegex(ValueError, 'lacks hierarchy metadata: batch_columns'):
            self.diagnostics(partial)
        invalid = [
            self.REFUSED.replace('\n', '|' + self.METADATA + '\n'),
            self.INSTALLED.replace('refusal=None', 'refusal=Some(MemoryBudget)'),
            self.INSTALLED.replace('installed=1', 'installed=2'),
            self.INSTALLED.replace('terminal=Direct', 'terminal=Unknown'),
            self.INSTALLED.replace('levels=2', 'levels=3'),
            self.INSTALLED.replace('levels=2', 'levels=0'),
            self.INSTALLED.replace('repeats=3;0', 'repeats=3'),
            self.INSTALLED.replace('repeats=3;0', 'repeats=3;-1'),
            self.INSTALLED.replace('hierarchy_nonzeros=448;41', 'hierarchy_nonzeros=448;x'),
            self.INSTALLED.replace('terminal_factor=1', 'terminal_factor=0'),
            self.INSTALLED.replace('terminal=Direct', 'terminal=MaximumLevels'),
            self.INSTALLED.replace('terminal_factor_dimension=8', 'terminal_factor_dimension=10'),
            self.INSTALLED.replace('parallel_operators=0', 'parallel_operators=2'),
            self.INSTALLED.replace('application=scalar', 'application=batched'),
            self.INSTALLED.replace('batch_columns=1', 'batch_columns=0'),
        ]
        for log in invalid:
            with self.subTest(log=log), self.assertRaises(ValueError):
                self.diagnostics(log)


class ModelDiscreteSignature(unittest.TestCase):
    def test_full_model_matrices_presence_flags_and_discrete_dof_are_included(self):
        raw = result('auto')
        original = science.model_discrete_sha256(raw)
        changes = [(name,0) for name in ('RSTATUS','RFE','RCLUSTER','RSUBSET','RMARKB','RMARKV','RREC')]
        changes += [(name,0) for name in ('RFIT','RDOF','RCERT')]
        changes += [('RDOF',2*row+1) for row in range(20) if row not in {4,11}]
        for name,index in changes:
            changed = copy.deepcopy(raw);changed['matrices'][name]['bits'][index] = bits(2.)
            with self.subTest(name=name,index=index):
                self.assertNotEqual(science.model_discrete_sha256(changed),original)

    def test_route_continuous_fields_excluded_and_numeric_signed_zero_equal(self):
        raw = result('auto')
        raw['matrices']['RDOF']['bits'][3] = bits(0.)
        changed = copy.deepcopy(raw)
        for name,index in [('RFIT',1),('RCERT',1),('RDOF',9),('RDOF',23),('RSOLVER',1),('RRUNTIME',39),('RB',0)]:
            changed['matrices'][name]['bits'][index] = bits(2.)
        changed['matrices']['RDOF']['bits'][3] = bits(-0.)
        self.assertEqual(science.model_discrete_sha256(raw),science.model_discrete_sha256(changed))
        # Model tables compare raw bits; selected RDOF cells compare numbers.
        raw['matrices']['RMARKB']['bits'][0] = bits(0.)
        changed = copy.deepcopy(raw);changed['matrices']['RMARKB']['bits'][0] = bits(-0.)
        self.assertNotEqual(science.model_discrete_sha256(raw),science.model_discrete_sha256(changed))

    def test_invalid_raw_or_model_shape_is_rejected(self):
        for mutate in [lambda raw:raw.update(return_code=True),
                       lambda raw:raw['matrices']['RDOF'].update(rows=10,columns=4),
                       lambda raw:raw['matrices']['RSTATUS'].update(rows=True),
                       lambda raw:raw['matrices'].pop('RMARKB')]:
            raw = result('auto');mutate(raw)
            with self.subTest(mutate=mutate),self.assertRaises(ValueError):science.model_discrete_sha256(raw)


class ResourceCertificates(unittest.TestCase):
    def test_actual_resource_cap_and_no_fe_exception_are_not_inferred(self):
        for trace in ['', TRACE.replace('memory_bytes=64', 'memory_bytes=129'),
                      TRACE.replace('certified=1', 'certified=0')]:
            with self.assertRaises(ValueError):
                science.resource_diagnostics(trace, 'h4-k0', False, {})
        with self.assertRaises(ValueError):
            science.resource_diagnostics('', 'h4-k0', True, {})
        allowed = science.resource_diagnostics('', 'h4-k0', True, {**{}, 'no_fe_exact_path': {}})
        self.assertEqual(allowed['resource_basis'], 'bound exact-no-FE witness')
        self.assertEqual(science.resource_diagnostics('', None, False, {})['resource_checks'],
                         {'iteration_caps': 'passed', 'memory_caps': 'passed'})
        # Retired construction-work fields are not required and carry no cap.
        legacy = TRACE.replace('memory_bytes=64', 'setup_work=12|setup_limit=32|memory_bytes=64')
        self.assertNotIn('setup_work_used', science.resource_diagnostics(
            legacy, 'h4-k0', False, {})['action_resources'][0])


    def test_actual_preset_and_handoff_parameters_must_match(self):
        with self.assertRaisesRegex(ValueError, 'requested preset'):
            science.resource_diagnostics(TRACE, 'h8-k0', False, {})
        wrong = TRACE + 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=32|samples=2\n'
        with self.assertRaisesRegex(ValueError, 'does not match'):
            science.resource_diagnostics(wrong, 'h4-k0', False, {})


    def test_portfolio_resources_validate_nested_two_fe_policy_and_all_caps(self):
        action = TRACE.replace('routing_h4_k0', 'routing_portfolio_v1')
        route = 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=1\n'
        for log in [action, action + route]:
            checked = science.resource_diagnostics(log, routing.PORTFOLIO_V1, False, {})
            self.assertEqual(checked['resource_checks'], {'iteration_caps': 'passed',
                'memory_caps': 'passed'})
            self.assertEqual(checked['action_resources'][0]['preset'], 'routing_portfolio_v1')
        for invalid in ['', route, action + route.replace('checkpoint=16', 'checkpoint=8'),
                        action + route.replace('samples=1', 'samples=0'),
                        action.replace('iterations=5', 'iterations=101'),
                        action.replace('memory_bytes=64', 'memory_bytes=129'),
                        action.replace('certified=1', 'certified=0'), TRACE]:
            with self.subTest(log=invalid), self.assertRaises(ValueError):
                science.resource_diagnostics(invalid, routing.PORTFOLIO_V1, False, {})


    def test_public_incremental_preset_and_every_action_are_certified(self):
        for family in ('many', 'two'):
            preset = 'routing_incremental_' + family
            action = TRACE.replace('routing_h4_k0', preset)
            result = science.resource_diagnostics(action, 'incremental-' + family, False, {}, expected_preset=preset)
            self.assertEqual(result['resource_checks'],
                             {'iteration_caps': 'passed', 'memory_caps': 'passed'})
            with self.assertRaisesRegex(ValueError, 'not certified'):
                science.resource_diagnostics(action + action.replace('certified=1', 'certified=0'),
                                            'incremental-' + family, False, {}, expected_preset=preset)


class IndependentWitnesses(unittest.TestCase):
    def test_replay_oracle_checks_published_columns(self):
        data = np.array([[1., 1.], [.2, -.2], [0., 0.], [.3, .4], [.5, .2], [.2, -.2]])
        outputs = dict(zip(['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed'], range(6)))
        expected = {'schema': 1, 'return_code': 0, 'poisson': False, 'rows': [0, 1],
                    'beta': [1.], 'covariance': [[1.]], 'sample': [1., 1.],
                    'residual': [.2, -.2], 'fitted': [.5, .2], 'outputs': outputs, 'saved': []}
        meta = {'rows': 2, 'columns': 6, 'outputs': outputs, 'replay_reference': expected}
        with tempfile.TemporaryDirectory() as directory:
            columns = Path(directory) / 'columns'
            data.tofile(columns)
            science.check_oracle(result(), columns, meta)
            expected['residual'][0] = .3
            with self.assertRaisesRegex(ValueError, 'independent replay oracle'):
                science.check_oracle(result(), columns, meta)

    def test_unseeded_scope_requires_typed_metadata_and_an_exact_policy(self):
        raw = UnseededPortfolioScope.raw()
        for field, value in [('absorbed_terms', True), ('absorbed_terms', 3.), ('categorical_intercepts', 1)]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                science.portfolio_scope(raw, raw, {**UnseededPortfolioScope.META, field: value}, 'auto',
                                        routing.policy(routing.PORTFOLIO_UNSEEDED))
        for field, value in [('injection', 'routing_portfolio_v1'), ('steps', False), ('certificate', 'requested')]:
            selected = routing.policy(routing.PORTFOLIO_UNSEEDED)
            selected[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                science.portfolio_scope(raw, raw, UnseededPortfolioScope.META, 'auto', selected)


if __name__ == '__main__':
    unittest.main()
