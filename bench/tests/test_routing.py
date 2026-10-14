"""Finite routing selection, immutable holdout and full failed-cell witnesses."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import struct
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_cases
from cmgschur_cases import additional_case, dense_oracle
from time_exact import compare
from test_native import case_bytes
from native_io import case_header


def h(text):
    return hashlib.sha256(text.encode()).hexdigest()


def manifest(samples=()):
    candidates = ['baseline'] + [f'h{n}-k{k}' for k in [0, *samples] for n in routing.CHECKPOINTS]
    arms = {a: {'sha256': h('baseline' if a == 'baseline' else 'candidate'),
                'policy': routing.policy(a), 'role': 'required_reference' if a == 'baseline' else 'candidate'} for a in candidates}
    arms['lsmr'] = {'sha256': h('candidate'), 'role': 'optional_comparator'}
    return {'schema': 2, 'failure_policy': copy.deepcopy(routing.FAILURE_POLICY), 'warmups': 1, 'repetitions': dict(routing.REPETITIONS),
            'arms': arms, 'candidates': candidates, 'protections': ['baseline'],
            'harness_sha256': h('harness'),
            'sampled_qualification': {str(k): h(f'science{k}') for k in samples},
            'panels': {stage: [{'id': stage + '-cell', 'case_sha256': h(stage), 'body_sha256': h('body' + stage), 'family': 'test-only',
                       'threads': 1, 'comparators': ['lsmr'], 'coverage': sorted(routing.COVERAGE | (routing.SAMPLED_COVERAGE if samples else set()))}]
                       for stage in routing.REPETITIONS}}


def records(spec, stage, previous=None, times=None):
    candidates = routing.required_candidates(spec, stage, previous)
    arms = set(candidates + spec['protections'] + [a for c in spec['panels'][stage] for a in c['comparators']])
    rows = []
    for cell in spec['panels'][stage]:
        for arm in sorted(arms):
            for repetition in range(-1, routing.REPETITIONS[stage]):
                rows.append({'cell': cell['id'], 'arm': arm, 'repetition': repetition,
                             'status': 'passed', 'seconds': (times or {}).get(arm, 1.),
                             'certificate_pass': True, 'science_pass': True,
                             'prepared_sha256': h('prepared-' + arm),
                             'resource_checks': {'iteration_caps': 'passed', 'memory_caps': 'passed',
                                                 'setup_caps': 'passed'},
                             'science_sha256': h(arm), 'witness_sha256': h('witness' + arm)})
    return {'status': 'complete', 'experiment_kind': spec.get('experiment_kind', 'routing'), 'failure_policy': copy.deepcopy(spec['failure_policy']), 'stage': stage, 'manifest_sha256': routing.identity(spec),
            'selection_sha256': routing.identity(previous) if previous else None,
            'artifacts': {a: spec['arms'][a]['sha256'] for a in sorted(arms)},
            'cases': {c['id']: c['case_sha256'] for c in spec['panels'][stage]},
            'harness_sha256': spec['harness_sha256'], 'records': rows,
            'prerequisites': [copy.deepcopy(row) for row in rows if row['repetition'] == -1]}


def prefix_manifest():
    original = manifest()
    spec = copy.deepcopy(original)
    spec.update(experiment_kind='prefix', prior_manifests=[routing.seal(original)],
                candidates=['baseline', *routing.PREFIXES], protections=['baseline', 'incumbent'])
    spec['arms'] = {a: original['arms'][a] for a in ['baseline', 'lsmr']}
    spec['arms']['incumbent'] = {'sha256': h('candidate'), 'policy': None, 'role': 'required_reference'}
    for arm in routing.PREFIXES:
        spec['arms'][arm] = {'sha256': h('candidate'), 'policy': routing.policy(arm), 'role': 'candidate'}
    for stage, cells in spec['panels'].items():
        cells[0].update(case_sha256=h('prefix-case-' + stage), body_sha256=h('prefix-body-' + stage),
                        coverage=sorted(routing.PREFIX_COVERAGE), categorical_intercepts=True, absorbed_terms=3)
    return spec


def overlap_manifest():
    original = manifest()
    times = {'h8-k0': .8}
    a = routing.evaluate(original, 'A', records(original, 'A', times=times))
    b = routing.evaluate(original, 'B', records(original, 'B', a, times), a)
    c = routing.evaluate(original, 'C', records(original, 'C', b, times), b)
    assert c['promoted'] == 'h8-k0'
    spec = copy.deepcopy(original)
    spec.update(experiment_kind='overlap', prior_manifests=[routing.seal(original)],
                serial_qualification={'manifest': routing.seal(original), 'decision': routing.seal(c)},
                candidates=['serial', 'overlap'], protections=['baseline', 'serial'])
    spec['arms'] = {a: original['arms'][a] for a in ['baseline', 'lsmr']}
    spec['arms']['serial'] = {'sha256': h('candidate'), 'policy': routing.policy('h8-k0'), 'role': 'required_reference'}
    spec['arms']['overlap'] = {'sha256': h('candidate'), 'policy': routing.policy('overlap-h8-k0'), 'role': 'candidate'}
    for stage, cells in spec['panels'].items():
        cells[0].update(case_sha256=h('overlap-case-' + stage), body_sha256=h('overlap-body-' + stage), threads=4)
    return spec


class RoutingTournament(unittest.TestCase):
    def setUp(self):
        self.spec = manifest()

    def test_framed_route_injection_changes_options_only_and_baseline_is_plain_auto(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'source.case'
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=cmgschur',
                                           b'cmgsamples=2', b'cmgseed=5', b'cmgdegree=4']))
            for arm in ['baseline', 'h32-k2']:
                target = root / (arm + '.case')
                routing.prepare(source, target, arm, 4)
                with target.open('rb') as stream:
                    _, args = case_header(stream)
                    self.assertEqual(stream.read(), b'body\n9\nthreads=1\n')
                fields = dict(a.decode().split('=', 1) for a in args[1:])
                self.assertEqual(fields['solver'], 'auto')
                self.assertEqual(fields['threads'], '4')
                self.assertNotIn('cmgsamples', fields)
                self.assertEqual(fields.get('inject'), None if arm == 'baseline' else 'routing_h32_k2')
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'inject=allocation']))
            with self.assertRaisesRegex(ValueError, 'already contains'):
                routing.prepare(source, root / 'rejected.case', 'h4-k0', 1)
            self.assertFalse((root / 'rejected.case').exists())

    def test_prefix_is_one_isolated_three_challenger_experiment(self):
        spec = prefix_manifest()
        routing.validate_manifest(spec)
        a = routing.evaluate(spec, 'A', records(spec, 'A'))
        self.assertEqual(a['shortlist'], ['baseline', 'prefix-none', 'prefix-map4', 'prefix-it4'])
        b = routing.evaluate(spec, 'B', records(spec, 'B', a), a)
        self.assertEqual(b['selected'], 'baseline')
        self.assertEqual(routing.policy('prefix-it4')['steps'], 4)
        self.assertEqual(routing.policy('prefix-it4')['map_sweeps'], 8)
        self.assertTrue(routing.policy('prefix-none')['replace_existing_prefix'])
        for invalid in ['stacked', 'mixed', 'two-fe', 'recycled', 'injected-incumbent']:
            bad = copy.deepcopy(spec)
            if invalid == 'stacked':
                bad['arms']['prefix-map4']['policy']['replace_existing_prefix'] = False
            elif invalid == 'mixed':
                bad['candidates'].append('h8-k0')
                bad['arms']['h8-k0'] = self.spec['arms']['h8-k0']
            elif invalid == 'two-fe':
                bad['panels']['B'][0]['absorbed_terms'] = 2
            elif invalid == 'injected-incumbent':
                bad['arms']['incumbent']['policy'] = routing.policy('prefix-none')
            else:
                bad['panels']['A'][0]['body_sha256'] = self.spec['panels']['C'][0]['body_sha256']
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                routing.validate_manifest(bad)

    def test_overlap_is_binary_for_a_fully_qualified_frozen_serial_policy(self):
        spec = overlap_manifest()
        routing.validate_manifest(spec)
        a = routing.evaluate(spec, 'A', records(spec, 'A'))
        self.assertEqual(a['shortlist'], ['serial', 'overlap'])
        b = routing.evaluate(spec, 'B', records(spec, 'B', a), a)
        self.assertEqual(b['selected'], 'serial')
        changed = {'overlap': .8}
        a = routing.evaluate(spec, 'A', records(spec, 'A', times=changed))
        b = routing.evaluate(spec, 'B', records(spec, 'B', a, changed), a)
        self.assertEqual(b['selected'], 'overlap')
        c = routing.evaluate(spec, 'C', records(spec, 'C', b, {'overlap': 1.06}), b)
        self.assertIsNone(c['promoted'])
        self.assertEqual(c['frozen_policy'], 'overlap')
        self.assertEqual(c['status'], 'not_qualified')

    def test_overlap_rejects_unqualified_or_changed_serial_basis_and_shared_data(self):
        original = overlap_manifest()
        for invalid in ['b-only', 'blocked-c', 'changed-failure-policy', 'baseline-policy',
                        'changed-policy', 'changed-artifact', 'one-worker', 'recycled', 'missing-chain', 'extra-arm']:
            spec = copy.deepcopy(original)
            if invalid == 'b-only':
                value = routing.unseal(spec['serial_qualification']['decision'])
                value.update(stage='B', status='selected')
                spec['serial_qualification']['decision'] = routing.seal(value)
            elif invalid in {'blocked-c', 'changed-failure-policy'}:
                value = routing.unseal(spec['serial_qualification']['decision'])
                if invalid == 'blocked-c':
                    value['blocking_references'] = ['baseline']
                else:
                    value['failure_policy']['per_call_timeout_s'] = 240
                spec['serial_qualification']['decision'] = routing.seal(value)
            elif invalid == 'baseline-policy':
                spec['arms']['baseline']['policy'] = routing.policy('prefix-map4')
            elif invalid == 'changed-policy':
                spec['arms']['overlap']['policy'] = routing.policy('overlap-h16-k0')
            elif invalid == 'changed-artifact':
                spec['arms']['serial']['sha256'] = spec['arms']['overlap']['sha256'] = h('new build')
            elif invalid == 'one-worker':
                spec['panels']['C'][0]['threads'] = 1
            elif invalid == 'recycled':
                spec['panels']['B'][0]['body_sha256'] = self.spec['panels']['C'][0]['body_sha256']
            elif invalid == 'missing-chain':
                spec['prior_manifests'] = []
            else:
                spec['candidates'].append('h16-k0')
                spec['arms']['h16-k0'] = self.spec['arms']['h16-k0']
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                routing.validate_manifest(spec)
        with self.assertRaisesRegex(ValueError, 'after four'):
            routing.policy('overlap-h4-k0')

    def test_compact_it_is_an_explicit_distinct_tuple_and_requires_declared_categorical_cells(self):
        spec = copy.deepcopy(self.spec)
        spec['arms'][routing.COMPACT_IT] = {'sha256': h('candidate'), 'solver': 'it',
            'injection': 'it_coefficients', 'role': 'optional_comparator'}
        for cells in spec['panels'].values():
            cells[0].update(categorical_intercepts=True, absorbed_terms=1)
            cells[0]['comparators'].append(routing.COMPACT_IT)
        routing.validate_manifest(spec)
        for invalid in ['renamed-it', 'wrong-solver', 'slopes']:
            bad = copy.deepcopy(spec)
            if invalid == 'renamed-it':
                bad['arms']['it'] = bad['arms'].pop(routing.COMPACT_IT)
            elif invalid == 'wrong-solver':
                bad['arms'][routing.COMPACT_IT]['solver'] = 'auto'
            else:
                bad['panels']['C'][0]['categorical_intercepts'] = False
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                routing.validate_manifest(bad)

    def test_new_presets_preserve_model_body_and_do_not_stack_injections(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'source.case'
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=auto']))
            for name in [*routing.PREFIXES, 'overlap-h8-k1', routing.COMPACT_IT]:
                target = root / (name + '.case')
                routing.prepare(source, target, name, 4)
                with target.open('rb') as stream:
                    _, args = case_header(stream)
                    self.assertEqual(stream.read(), b'body\n9\nthreads=1\n')
                fields = dict(a.decode().split('=', 1) for a in args[1:])
                self.assertEqual(fields['solver'], 'it' if name == routing.COMPACT_IT else 'auto')
                self.assertEqual(fields['inject'], 'it_coefficients' if name == routing.COMPACT_IT else routing.policy(name)['injection'])
                with self.assertRaisesRegex(ValueError, 'already contains'):
                    routing.prepare(target, root / ('stacked-' + name), name, 4)

    def test_finite_grid_and_qualified_sampling_only(self):
        routing.validate_manifest(self.spec)
        sampled = manifest((1, 2))
        self.assertEqual(len(sampled['candidates']), 13)
        routing.validate_manifest(sampled)
        missing_stress = copy.deepcopy(sampled)
        missing_stress['panels']['A'][0]['coverage'].remove('high-degree-weak')
        with self.assertRaisesRegex(ValueError, 'missing coverage'):
            routing.validate_manifest(missing_stress)
        sampled['sampled_qualification'].pop('2')
        with self.assertRaisesRegex(ValueError, 'full four-checkpoint grid'):
            routing.validate_manifest(sampled)
        self.assertEqual(routing.policy('h32-k2')['injection'], 'routing_h32_k2')
        for arm in ['auto', 'h2-k0', 'h4-k4', 'baseline-h4-k0']:
            with self.assertRaises(ValueError):
                routing.policy(arm)

    def test_manifest_rejects_overlap_missing_coverage_or_changed_policy(self):
        for change in ['overlap', 'body-overlap', 'coverage', 'policy', 'artifact']:
            spec = copy.deepcopy(self.spec)
            if change == 'overlap':
                spec['panels']['C'][0]['case_sha256'] = spec['panels']['B'][0]['case_sha256']
            elif change == 'body-overlap':
                spec['panels']['C'][0]['body_sha256'] = spec['panels']['B'][0]['body_sha256']
            elif change == 'coverage':
                spec['panels']['A'][0]['coverage'].remove('one-fe')
            elif change == 'policy':
                spec['arms']['h4-k0']['policy']['degree'] = 4
            else:
                spec['arms']['h4-k0']['sha256'] = h('different')
            with self.subTest(change=change), self.assertRaises(ValueError):
                routing.validate_manifest(spec)

    def test_a_lexicographic_shortlist_and_conservative_ties(self):
        output = routing.evaluate(self.spec, 'A', records(self.spec, 'A'))
        self.assertEqual(output['shortlist'], ['baseline', 'h32-k0', 'h16-k0', 'h8-k0'])
        # A faster candidate with a baseline violation cannot outrank one without it.
        values = {'baseline': 1., 'h4-k0': 1.051, 'h8-k0': .90, 'h16-k0': .95, 'h32-k0': .96}
        output = routing.evaluate(self.spec, 'A', records(self.spec, 'A', times=values))
        self.assertEqual(output['shortlist'], ['h8-k0', 'h16-k0', 'h32-k0', 'baseline'])

    def test_missing_timeout_science_repeat_and_resource_failures_are_infinite(self):
        for change in ['missing', 'timeout', 'repeat', 'witness', 'science', 'certificate', 'memory', 'cap']:
            data = records(self.spec, 'A')
            index = next(i for i, r in enumerate(data['records']) if r['arm'] == 'h32-k0')
            row = data['records'][index]
            if change == 'missing':
                data['records'].pop(index)
            elif change == 'timeout':
                row['status'] = 'blocked'
                row.pop('seconds')
            elif change == 'repeat':
                row['science_sha256'] = h('changed repeat')
            elif change == 'witness':
                row['witness_sha256'] = h('changed witness')
            elif change == 'science':
                row['science_pass'] = False
            elif change == 'certificate':
                row['certificate_pass'] = False
            elif change == 'memory':
                row['resource_checks']['memory_caps'] = 'unchecked'
            else:
                row['action_resources'] = [{'iterations_used': 11, 'iterations_limit': 10}]
            with self.subTest(change=change):
                output = routing.evaluate(self.spec, 'A', data)
                failed = next(r for r in output['ranking'] if r['arm'] == 'h32-k0')
                self.assertIsNone(failed['worst_internal_normalized'])
                self.assertEqual(len(output['ranking']), 5)
                self.assertEqual(output['failures'][0]['availability_loss'], 'unresolved')
                self.assertNotIn('h32-k0', output['shortlist'])

    def test_failed_a_cells_never_advance_even_when_shortlist_has_room(self):
        data = records(self.spec, 'A')
        for row in data['records']:
            if row['arm'] != 'lsmr':
                row['status'] = 'blocked'
        result = routing.evaluate(self.spec, 'A', data)
        self.assertEqual(result['status'], 'no_qualifying_policy')
        self.assertEqual(result['shortlist'], [])
        self.assertEqual(len(result['ranking']), 5)

    def test_missing_stage_incumbent_prevents_a_advancement(self):
        self.spec['arms']['incumbent'] = {'sha256': h('incumbent'), 'role': 'required_reference'}
        self.spec['protections'].append('incumbent')
        data = records(self.spec, 'A')
        data['records'] = [r for r in data['records'] if r['arm'] != 'incumbent']
        result = routing.evaluate(self.spec, 'A', data)
        self.assertEqual(result['status'], 'no_qualifying_policy')
        self.assertEqual(result['shortlist'], [])
        self.assertTrue(all(not r['data_valid'] for r in result['ranking']))

    def test_every_comparator_is_required_no_failed_cell_deletion(self):
        a = routing.evaluate(self.spec, 'A', records(self.spec, 'A'))
        b = records(self.spec, 'B', a)
        b['records'] = [r for r in b['records'] if r['arm'] != 'lsmr']
        result = routing.evaluate(self.spec, 'B', b, a)
        self.assertEqual(result['status'], 'no_qualifying_policy')
        self.assertIsNone(result['selected'])
        self.assertEqual(len(result['ranking']), 4)

    def optional_failure(self, data, *, measured=0, timeout=False):
        rows = [r for r in data['records'] if r['arm'] == 'lsmr']
        failure = rows[measured + 1]
        failure.update(status='unavailable', failure={
            'kind': 'timeout', 'dispatched': True, 'log_sha256': h('actual log'),
            'prepared_sha256': failure['prepared_sha256'], 'call_limit_s': 120., 'elapsed_s': 120.})
        failure.pop('seconds')
        for repetition, row in enumerate(rows[measured + 2:], measured + 1):
            row.clear()
            row.update(cell=failure['cell'], arm='lsmr', repetition=repetition,
                       status='not_run', blocked_by_sha256=routing.observation_identity(failure))
        return failure

    def test_observed_optional_timeout_is_not_a_candidate_failure_or_a_runtime_statistic(self):
        data = records(self.spec, 'A')
        self.optional_failure(data)
        output = routing.evaluate(self.spec, 'A', data)
        self.assertEqual(output['shortlist'], ['baseline', 'h32-k0', 'h16-k0', 'h8-k0'])
        for candidate in output['ranking']:
            comparison = candidate['comparisons'][1]
            self.assertEqual(comparison['status'], 'availability_dominance')
            self.assertIsNone(comparison['pass'])
            self.assertFalse(comparison['runtime_identified'])
            self.assertNotIn('median_ratio', comparison)
            self.assertNotIn('bootstrap95', comparison)
            self.assertIsNone(candidate['worst_internal_normalized'])
            self.assertFalse(candidate['frontier_coverage']['complete_runtime_frontier'])
        self.assertEqual(output['failures'][0]['status'], 'unavailable')

    def test_partial_optional_repetitions_are_never_filtered_into_a_runtime(self):
        data = records(self.spec, 'A')
        self.optional_failure(data, measured=1)
        output = routing.evaluate(self.spec, 'A', data)
        self.assertEqual(output['failures'][0]['successful_calls'], 3)  # prerequisite, warmup, one measurement
        self.assertTrue(all(r['comparisons'][1]['status'] == 'availability_dominance' for r in output['ranking']))

    def test_unknown_failure_missing_rows_bad_science_and_aggregate_timeout_block_the_panel(self):
        for kind in ['unknown', 'missing', 'science', 'aggregate', 'unbound', 'retry']:
            data = records(self.spec, 'A')
            failure = self.optional_failure(data)
            if kind == 'unknown':
                failure['failure']['kind'] = 'unknown_native_failure'
            elif kind == 'missing':
                data['records'].remove(failure)
            elif kind == 'science':
                failure['status'] = 'blocked'
            elif kind == 'aggregate':
                failure['failure']['call_limit_s'] = 119.
            elif kind == 'unbound':
                next(r for r in data['records'] if r['status'] == 'not_run')['blocked_by_sha256'] = h('unknown')
            else:
                row = next(r for r in data['records'] if r['status'] == 'not_run')
                row.update(copy.deepcopy(next(r for r in records(self.spec, 'A')['records']
                                              if r['arm'] == 'lsmr' and r['repetition'] == row['repetition'])))
            with self.subTest(kind=kind):
                output = routing.evaluate(self.spec, 'A', data)
                self.assertEqual(output['shortlist'], [])
                self.assertTrue(output['blocking_references'])

    def test_required_reference_observed_timeout_still_blocks_nonregression(self):
        data = records(self.spec, 'A')
        self.optional_failure(data)
        for row in data['records']:
            if row['arm'] == 'baseline':
                row['status'] = 'blocked'
        output = routing.evaluate(self.spec, 'A', data)
        self.assertEqual(output['shortlist'], [])
        self.assertTrue(any(r['role'] == 'required_reference' for r in output['blocking_references']))

    def test_manifest_roles_policy_and_legacy_records_cannot_silently_change(self):
        for change in ['schema', 'roles', 'failure_policy']:
            spec = copy.deepcopy(self.spec)
            if change == 'schema':
                spec['schema'] = 1
            elif change == 'roles':
                spec['arms']['lsmr']['role'] = 'candidate'
            else:
                spec['failure_policy']['per_call_timeout_s'] = 121.
            with self.subTest(change=change), self.assertRaises(ValueError):
                routing.validate_manifest(spec)
        data = records(self.spec, 'A')
        data.pop('failure_policy')
        with self.assertRaisesRegex(ValueError, 'failure policy changed'):
            routing.evaluate(self.spec, 'A', data)

    def test_b_upper_bounds_and_c_cannot_switch_policy_or_resample(self):
        values = {'h4-k0': .85, 'h8-k0': .86, 'h16-k0': .87, 'h32-k0': .88}
        a = routing.evaluate(self.spec, 'A', records(self.spec, 'A', times=values))
        b = routing.evaluate(self.spec, 'B', records(self.spec, 'B', a, values), a)
        self.assertEqual(b['selected'], 'h4-k0')
        c = records(self.spec, 'C', b, {'h4-k0': 1.06})
        final = routing.evaluate(self.spec, 'C', c, b)
        self.assertEqual(final['frozen_policy'], 'h4-k0')
        self.assertEqual(final['status'], 'not_qualified')
        self.assertIsNone(final['promoted'])
        c['selection_sha256'] = h('another policy')
        with self.assertRaisesRegex(ValueError, 'selection frozen'):
            routing.evaluate(self.spec, 'C', c, b)
        with self.assertRaises(ValueError):
            routing.evaluate(self.spec, 'C', records(self.spec, 'A'), a)

    def test_boolean_measurements_and_workers_rejected(self):
        data = records(self.spec, 'A')
        data['records'][0]['seconds'] = True
        with self.assertRaises(ValueError):
            routing.evaluate(self.spec, 'A', data)
        self.spec['panels']['A'][0]['threads'] = True
        with self.assertRaises(ValueError):
            routing.validate_manifest(self.spec)

    def test_perfect_baseline_self_comparison_cannot_hide_internal_regret(self):
        a = routing.evaluate(self.spec, 'A', records(self.spec, 'A'))
        b = routing.evaluate(self.spec, 'B', records(self.spec, 'B', a, {'lsmr': .5}), a)
        self.assertEqual(b['status'], 'no_qualifying_policy')
        baseline = next(r for r in b['ranking'] if r['arm'] == 'baseline')
        self.assertTrue(baseline['comparisons'][0]['pass'])
        self.assertFalse(baseline['comparisons'][1]['pass'])

    def test_b_protects_incumbent_and_c_rejects_an_unselected_arm(self):
        self.spec['arms']['incumbent'] = {'sha256': h('incumbent'), 'role': 'required_reference'}
        self.spec['protections'].append('incumbent')
        a = routing.evaluate(self.spec, 'A', records(self.spec, 'A'))
        failed = routing.evaluate(self.spec, 'B', records(self.spec, 'B', a, {'incumbent': .5}), a)
        self.assertEqual(failed['status'], 'no_qualifying_policy')
        b = routing.evaluate(self.spec, 'B', records(self.spec, 'B', a), a)
        c = records(self.spec, 'C', b)
        c['records'].append({**c['records'][0], 'arm': 'h4-k0'})
        with self.assertRaisesRegex(ValueError, 'unexpected'):
            routing.evaluate(self.spec, 'C', c, b)

    def test_internal_allowance_is_twenty_percent_or_one_millisecond(self):
        values = {'baseline': .001, 'lsmr': .001, 'h4-k0': .0019}
        a = routing.evaluate(self.spec, 'A', records(self.spec, 'A', times=values))
        b = routing.evaluate(self.spec, 'B', records(self.spec, 'B', a, values), a)
        for row in b['ranking']:
            for comparison in row['comparisons']:
                self.assertEqual(comparison['upper_allowance'], 2.)
        self.assertIn('no simultaneous claim', b['confidence'])
        self.assertEqual(compare([1.]*7, [1.19]*7, relative_allowance=.20)['nonregression_pass'], True)
        self.assertEqual(compare([1.]*7, [1.19]*7)['nonregression_pass'], False)

    def test_excess_regret_preserves_negative_speedups_across_absolute_allowances(self):
        second = copy.deepcopy(self.spec['panels']['B'][0])
        second.update(id='B-tiny', case_sha256=h('B-tiny'), body_sha256=h('B-tiny-body'))
        self.spec['panels']['B'].append(second)
        a = routing.evaluate(self.spec, 'A', records(self.spec, 'A', times={'h4-k0': .8, 'h8-k0': .9}))
        data = records(self.spec, 'B', a, {'h4-k0': .8, 'h8-k0': .9})
        for row in data['records']:
            if row['cell'] == 'B-tiny':
                row['seconds'] = {'h4-k0': .0009, 'h8-k0': .0007}.get(row['arm'], .001)
        b = routing.evaluate(self.spec, 'B', data, a)
        scores = {r['arm']: r['worst_internal_normalized'] for r in b['ranking']}
        self.assertAlmostEqual(scores['h4-k0'], -.1)
        self.assertAlmostEqual(scores['h8-k0'], -.3)
        self.assertEqual(b['selected'], 'h8-k0')

    def test_noise_upper_bound_not_median_controls_b(self):
        a = routing.evaluate(self.spec, 'A', records(self.spec, 'A'))
        data = records(self.spec, 'B', a)
        rows = [r for r in data['records'] if r['arm'] == 'h32-k0' and r['repetition'] >= 0]
        for row, seconds in zip(rows, [.7, .8, .9, 1., 1.1, 1.2, 1.3]):
            row['seconds'] = seconds
        output = routing.evaluate(self.spec, 'B', data, a)
        candidate = next(r for r in output['ranking'] if r['arm'] == 'h32-k0')
        self.assertFalse(candidate['pass'])
        self.assertEqual(candidate['comparisons'][0]['median_ratio'], 1.)
        self.assertGreater(candidate['comparisons'][0]['bootstrap95'][1], 1.05)

    def test_seals_no_overwrite_duplicates_and_bad_times(self):
        envelope = routing.seal(self.spec)
        envelope['value']['warmups'] = 2
        with self.assertRaisesRegex(ValueError, 'changed'):
            routing.unseal(envelope)
        for base, candidate in [([], []), ([1.], [1., 2.]), ([0.], [1.]), ([1.], [float('nan')])]:
            with self.assertRaises(ValueError):
                compare(base, candidate)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            spec = root / 'spec.json'
            spec.write_text(json.dumps(manifest()))
            output = root / 'frozen.json'
            argv = ['routing', 'freeze', '--spec', str(spec), '--output', str(output)]
            with patch.object(sys, 'argv', argv), patch.object(routing, 'scratch', side_effect=lambda p: p):
                self.assertEqual(routing.main(), 0)
                with self.assertRaises(FileExistsError):
                    routing.main()
            spec.write_text('{"schema":1,"schema":1}')
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                routing.read_json(spec)

    def test_imported_family_coverage_checks_canonical_terms_not_filenames(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / 'claimed-slopes.case'
            for family, count, term in [('no-fe', 0, [0., 0., 0., 0.]),
                                         ('one-fe', 1, [1., 1., 1., 0.]),
                                         ('slopes', 1, [1., 1., 1., 1.])]:
                args = [b'estimate-ols', b'threads=1', b'output_saved_channels=1:2', f'absorb_term_count={count}'.encode(),
                        b'absorb_term_matrix=A'] + [f'output_{name}=1'.encode() for name in
                        ['sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed']]
                prefix = f'FEREG_REPLAY_V1\n2 2 {len(args)} 1 0\n'.encode()
                framed = b''.join(str(len(a)).encode() + b'\n' + a + b'\n' for a in args)
                data = b'0\n' + b'0000000000000000\n' * 2
                table = b'1\nA\n1 4\n' + b''.join(struct.pack('>d', v).hex().encode() + b'\n' for v in term)
                path.write_bytes(prefix + framed + data * 2 + table)
                inspected = routing_cases.inspect_case(path, family)
                self.assertEqual(inspected['absorbed_terms'], count)
                self.assertEqual(inspected['saved'], [1])
                for wrong in set(routing_cases.REQUIRED_IMPORTS) - {family}:
                    with self.subTest(actual=family, claimed=wrong), self.assertRaises(ValueError):
                        routing_cases.inspect_case(path, wrong)

    def test_extended_families_have_the_declared_mathematical_structure(self):
        columns, weights, count, intercept = routing_cases.extended_columns('disconnected', 19, 300, 100, 20)
        worker, firm = columns[-2:]
        self.assertTrue(np.array_equal(worker < 150, firm < 50))
        self.assertEqual((weights, count, intercept), (None, 2, True))
        columns, weights, count, _ = routing_cases.extended_columns('near-confounded-third', 19, 300, 100, 20)
        mismatch = np.mean(columns[-1] != columns[-2] % 20)
        self.assertGreater(mismatch, 0.)
        self.assertLess(mismatch, .02)
        self.assertEqual(count, 3)
        _, weights, _, _ = routing_cases.extended_columns('weight-dynamic', 19, 300, 100, 20)
        self.assertTrue(np.all(np.isfinite(weights)) and np.all(weights > 0))
        self.assertGreater(weights.max() / weights.min(), 1e6)
        columns, weights, count, intercept = routing_cases.extended_columns('inactive-columns', 19, 300, 100, 20)
        for values in columns[:-2]:
            self.assertTrue(np.array_equal(values.reshape(-1, 2).sum(axis=1), np.zeros(len(values)//2)))
        self.assertFalse(intercept)

    def test_sparse_generator_requires_explicit_unsupported_shapes(self):
        self.assertEqual(set(routing_cases.NATIVE_CASES) | set(routing_cases.SYNTHETIC_CASES)
                         | set(routing_cases.REQUIRED_IMPORTS), routing.COVERAGE)
        self.assertEqual(set(routing_cases.SAMPLED_CASES), routing.SAMPLED_COVERAGE)
        self.assertEqual(len(set(routing_cases.SEEDS.values())), 3)
        with self.assertRaisesRegex(ValueError, 'require distinct'):
            routing_cases.sources([])
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            paths = []
            for name in routing_cases.REQUIRED_IMPORTS:
                path = root / name
                path.write_text('input-' + name)
                paths.append(f'{name}={path}')
            self.assertEqual(set(routing_cases.sources(paths)), set(routing_cases.REQUIRED_IMPORTS))
            with self.assertRaisesRegex(ValueError, 'exactly once'):
                routing_cases.sources(paths + paths[:1])

    def test_comparator_eligibility_uses_the_imported_shape(self):
        choose = lambda family, count: routing_cases.comparators({'family': family, 'absorbed_terms': count}, True)
        self.assertEqual(choose('no-fe', 0), [])
        self.assertEqual(choose('one-fe', 1), [])
        self.assertEqual(choose('slopes', 1), ['lsmr', 'map', 'it'])
        self.assertEqual(choose('sixteen-fe', 16), ['lsmr', 'map', 'it'])
        self.assertEqual(choose('low-saved', 3), ['lsmr', 'map', 'it'])
        self.assertEqual(choose('low-saved', 2), ['lsmr', 'map', 'it'])
        self.assertEqual(choose('dense', 2), ['diagonal', 'lsmr', 'map', 'it', 'cmg',
                                               'cmgschur-k0', 'cmgschur-k1', 'cmgschur-k2'])

    def test_compact_it_generator_opt_in_preserves_public_it_and_excludes_slopes(self):
        for family, count, categorical in [('no-fe', 0, False), ('one-fe', 1, True),
                                            ('slopes', 1, False), ('dense', 2, True),
                                            ('sixteen-fe', 16, True)]:
            metadata = {'family': family, 'absorbed_terms': count,
                        'categorical_intercepts': categorical}
            original = routing_cases.comparators(metadata, sampled=True)
            expanded = routing_cases.comparators(metadata, sampled=True, compact_it=True)
            with self.subTest(family=family):
                self.assertEqual([arm for arm in expanded if arm != routing.COMPACT_IT], original)
                self.assertEqual(routing.COMPACT_IT in expanded, categorical and count >= 1)

    def test_panel_seed_override_binds_metadata_and_native_generator_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            generator = root / 'generator'
            generator.write_text('mock generator')
            for chosen in [None, 987654321]:
                output = root / str(chosen)
                args = ['routing_cases.py', '--panel', 'A', '--output', str(output),
                        '--generator', str(generator)]
                if chosen is not None:
                    args += ['--seed', str(chosen)]
                def generate(command, **kwargs):
                    Path(command[command.index('--output') + 1]).write_text('mock case')
                metadata = {'body_sha256': h('body'), 'absorbed_terms': 2,
                            'categorical_intercepts': True}
                with patch.object(sys, 'argv', args), patch('builtins.print'), \
                        patch.object(routing_cases, 'sources', return_value={}), \
                        patch.object(routing_cases, 'scratch', return_value=output), \
                        patch.object(routing_cases, 'NATIVE_CASES', {'dense': {'rows': 2}}), \
                        patch.object(routing_cases, 'SYNTHETIC_CASES', {}), \
                        patch.object(routing_cases, 'inspect_case', return_value=metadata), \
                        patch.object(routing_cases.subprocess, 'run', side_effect=generate) as native:
                    routing_cases.main()
                expected = routing_cases.SEEDS['A'] if chosen is None else chosen
                self.assertEqual(json.loads((output / 'panel.json').read_text())['seed'], expected)
                self.assertEqual(json.loads((output / 'a-dense.json').read_text())['seed'], expected)
                command = native.call_args.args[0]
                self.assertEqual(command[command.index('--seed') + 1], str(expected))

    def test_paired_construction_witness_agrees_with_independent_dense_dummies(self):
        columns = additional_case('orthogonal', 50, 20, 12, 41)
        witness = routing_cases.paired_orthogonal_oracle(columns)
        dense = dense_oracle(columns, include_intercept=False)
        self.assertEqual(witness['rank'], dense['rank'])
        for name in ['beta', 'covariance', 'residual', 'fitted']:
            np.testing.assert_allclose(witness[name], dense[name], rtol=1e-9, atol=1e-11)
        columns[0][0] += 1e-10
        with self.assertRaisesRegex(ValueError, 'not exact'):
            routing_cases.paired_orthogonal_oracle(columns)


if __name__ == '__main__':
    unittest.main()
