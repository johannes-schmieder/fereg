"""Prospective family qualification keeps legacy outcomes and explicit diagnostics separate."""
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_discovery as discovery
import routing_parallel as parallel
import routing_run
from test_routing import h, records
from test_routing_discovery import partition42, save_json
import test_routing_parallel as parallel_tests
import test_routing_run as run_tests
import json


def incremental_spec(family='many'):
    old = partition42()
    spec = copy.deepcopy(old)
    candidate = 'incremental-' + family
    spec.update(schema=3, experiment_kind='incremental', candidates=['incumbent', candidate],
                protections=['baseline', 'incumbent'], failure_policy=copy.deepcopy(routing.INCREMENTAL_FAILURE_POLICY),
                incremental={'family': family, 'sequence': 1 if family == 'many' else 2,
                             'original_revision': 'f35f3a4d', 'incumbent_revision': 'a' * 40})
    spec['arms'].pop(routing.PORTFOLIO_UNSEEDED)
    spec['arms']['incumbent'] = {'sha256': h('accepted'), 'policy': None, 'role': 'required_reference'}
    spec['arms'][candidate] = {'sha256': h('family'), 'policy': routing.policy(candidate), 'role': 'candidate'}
    for stage, cells in spec['panels'].items():
        for cell in cells:
            cell.setdefault('absorbed_terms', 3 if family == 'many' else 2)
            cell.setdefault('categorical_intercepts', True)
    return spec


def score_data(spec, *, candidate=.9, incumbent=1., baseline=1., explicit=.01):
    times, outcomes = {}, {}
    arm = routing.incremental_candidate(spec)
    for cell in spec['panels']['A']:
        for a in dict.fromkeys(spec['candidates'] + spec['protections'] + cell['comparators']):
            times[cell['id'], a] = [candidate if a == arm else incumbent if a == 'incumbent'
                                  else baseline if a == 'baseline' else explicit] * 9
            outcomes[cell['id'], a] = {'status': 'passed'}
    return times, outcomes


class IncrementalProtocolTests(unittest.TestCase):
    def test_freeze_validates_new_schema_dual_public_protection_and_family(self):
        spec = incremental_spec()
        self.assertIs(routing.validate_manifest(spec), spec)
        for key, value in [('schema', 2), ('protections', ['baseline']),
                           ('candidates', ['baseline', 'incremental-many']),
                           ('failure_policy', routing.FAILURE_POLICY)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                routing.validate_manifest({**spec, key: value})
        bad = copy.deepcopy(spec)
        bad['arms']['incumbent']['policy'] = routing.policy(routing.PORTFOLIO_UNSEEDED)
        with self.assertRaises(ValueError):
            routing.validate_manifest(bad)
        with self.assertRaises(ValueError):
            routing.validate_manifest(incremental_spec('two'))

    def test_second_family_preserves_preceding_full_manifest_freshness(self):
        prior, spec = incremental_spec(), incremental_spec('two')
        source = {'production_tokens_sha256': h('production'), 'artifact_inputs_sha256': h('build')}
        terminal = {'stage_family': 'many', 'status': 'retained', 'prior_manifest': routing.seal(prior),
                    'incumbent_revision': 'a' * 40, 'incumbent_source_identity': source,
                    'incumbent_artifacts': {'Linux': spec['arms']['incumbent']['sha256'], 'Darwin': h('mac')}}
        spec['incremental'].update(preceding_stage=routing.seal(terminal), incumbent_source_identity=source)
        for stage, cells in spec['panels'].items():
            for cell in cells:
                cell['case_sha256'] = h('next-case-' + stage + cell['id'])
                cell['body_sha256'] = h('next-body-' + stage + cell['id'])
        import routing_block_deployment
        with patch.object(routing_block_deployment, 'validate_terminal'):
            with self.assertRaisesRegex(ValueError, 'freshness chain'):
                routing.validate_manifest(spec)
            spec['prior_manifests'].append(routing.seal(prior))
            routing.validate_manifest(spec)
            spec['panels']['C'][0]['body_sha256'] = prior['panels']['B'][0]['body_sha256']
            with self.assertRaisesRegex(ValueError, 'reuses a prior'):
                routing.validate_manifest(spec)

    def test_eligibility_is_structural_and_policy_has_exact_injection(self):
        for count in range(0, 6):
            for categorical in (False, True):
                cell = {'absorbed_terms': count, 'categorical_intercepts': categorical}
                self.assertEqual(routing.incremental_eligible(cell, 'many'), categorical and count >= 3)
                self.assertEqual(routing.incremental_eligible(cell, 'incremental-two'), categorical and count == 2)
        self.assertFalse(routing.incremental_eligible({'absorbed_terms': True, 'categorical_intercepts': True}, 'two'))
        for arm in routing.INCREMENTAL:
            self.assertEqual(routing.policy_name(routing.policy(arm)), arm)
            self.assertEqual(routing.policy(arm)['injection'], 'routing_' + arm.replace('-', '_'))

    def test_explicit_speed_and_invalid_science_never_gate_or_rank(self):
        spec = incremental_spec()
        times, outcomes = score_data(spec)
        original = discovery.score(spec, times, outcomes)
        self.assertEqual(original[0]['arm'], 'incremental-many')
        self.assertTrue(all(r['pass'] for r in original))
        for cell in spec['panels']['A']:
            for arm in cell['comparators']:
                times[cell['id'], arm] = None
                outcomes[cell['id'], arm] = {'status': 'invalid'}
        invalid = discovery.score(spec, times, outcomes)
        self.assertEqual([(r['arm'], r['pass'], r['geomean_incumbent_ratio']) for r in original],
                         [(r['arm'], r['pass'], r['geomean_incumbent_ratio']) for r in invalid])
        self.assertTrue(all(not c['runtime_identified'] for r in invalid for c in r['comparisons'] if not c['protection']))
        self.assertEqual(parallel.incremental_screen(invalid, spec)['shortlist'], ['incumbent', 'incremental-many'])

    def test_both_auto_protections_and_validator_defect_are_gates(self):
        spec = incremental_spec()
        for original, current in ((.8, 1.), (1., .8)):
            times, outcomes = score_data(spec, baseline=original, incumbent=current)
            candidate = next(r for r in discovery.score(spec, times, outcomes) if r['arm'] == 'incremental-many')
            self.assertFalse(candidate['pass'])
            self.assertGreater(candidate['baseline_violations'], 0)
        times, outcomes = score_data(spec)
        key = next(k for k in outcomes if k[1] == 'lsmr')
        times[key], outcomes[key] = None, {'status': 'blocked', 'reason': 'validator defect'}
        self.assertTrue(all(not r['pass'] for r in discovery.score(spec, times, outcomes)))
        times, outcomes = score_data(spec)
        key = next(k for k in outcomes if k[1] == 'incremental-many')
        times[key], outcomes[key] = None, {'status': 'blocked'}
        self.assertFalse(next(r for r in discovery.score(spec, times, outcomes) if r['arm'] == key[1])['pass'])

    def test_equal_cell_weight_affected_family_only_and_incumbent_tie(self):
        spec = incremental_spec()
        times, outcomes = score_data(spec, candidate=1.)
        for cell in spec['panels']['A']:
            if not routing.incremental_eligible(cell, 'many'):
                times[cell['id'], 'incremental-many'] = [.1] * 9
        ranking = discovery.score(spec, times, outcomes)
        self.assertEqual(ranking[0]['arm'], 'incumbent')
        self.assertEqual([r['geomean_incumbent_ratio'] for r in ranking], [1., 1.])
        affected = [c for c in spec['panels']['A'] if routing.incremental_eligible(c, 'many')]
        times[affected[0]['id'], 'incremental-many'] = [.25] * 9
        ratio = next(r for r in discovery.score(spec, times, outcomes) if r['arm'] == 'incremental-many')['geomean_incumbent_ratio']
        self.assertAlmostEqual(ratio, .25 ** (1 / len(affected)))

    def test_partition_retains_six_allocations_and_fixed_inclusive_budget(self):
        spec = incremental_spec()
        parts = discovery.partitions(spec)
        self.assertEqual([len(p['cell_ids']) for p in parts], [20, 22])
        self.assertEqual(sum(p['arm_slots'] for p in parts), 402)
        self.assertEqual(3 * sum(p['maximum_native_calls'] for p in parts), 8442)
        self.assertEqual(3 * sum(p['inclusive_allocation_budget_s'] for p in parts), 43200)
        for part in parts:
            self.assertEqual(discovery.partition_spec(spec, part)['failure_policy']['aggregate_budget_s'],
                             part['inclusive_allocation_budget_s'])

    def test_legacy_selector_cannot_qualify_incremental_b_or_c(self):
        spec = incremental_spec()
        spec['panels']['A'] = [{**spec['panels']['B'][0], 'id': 'A-many',
                                'case_sha256': h('A-many'), 'body_sha256': h('A-many-body')}]
        a = routing.evaluate(spec, 'A', records(spec, 'A', times={'incremental-many': .9}))
        self.assertEqual(a['shortlist'][0], 'incremental-many')
        for stage in ('B', 'C'):
            with self.assertRaisesRegex(ValueError, 'Mac block protocol'):
                routing.evaluate(spec, stage, {}, a)

    def test_only_bound_known_comparator_science_failure_is_diagnostic(self):
        spec = incremental_spec()
        first = {'arm': 'lsmr', 'status': 'blocked', 'prepared_sha256': h('input'),
                 'failure': {'kind': 'comparator_science_invalid', 'dispatched': True, 'process_completed': True,
                             'result_sha256': h('raw'), 'log_sha256': h('log'), 'prepared_sha256': h('input')}}
        skipped = {'arm': 'lsmr', 'status': 'not_run', 'blocked_by_sha256': routing.observation_identity(first)}
        self.assertEqual(routing.arm_outcome(spec, [skipped], first)['status'], 'invalid')
        for change in ({'arm': 'incumbent'}, {'arm': 'incremental-many'},
                       {'failure': {**first['failure'], 'kind': 'validator_failed'}}):
            bad = {**first, **change}
            skip = {**skipped, 'blocked_by_sha256': routing.observation_identity(bad)}
            self.assertEqual(routing.arm_outcome(spec, [skip], bad)['status'], 'blocked')
        self.assertTrue(routing_run.comparator_scientific_mismatch('RB: tolerance failure (maximum absolute difference 1)'))
        self.assertFalse(routing_run.comparator_scientific_mismatch('invalid oracle row arrays'))
        self.assertFalse(routing_run.comparator_scientific_mismatch('binary output size mismatch'))


class IncrementalProducerTests(unittest.TestCase):
    run_panel = run_tests.RoutingProducer.run_panel

    def setUp(self):
        run_tests.RoutingProducer.setUp(self)
        self.spec = incremental_spec()
        self.cell.update(absorbed_terms=3, categorical_intercepts=True, body_sha256=h('incremental-producer-body'),
                         coverage=sorted(routing.PORTFOLIO_COVERAGE))
        self.spec['panels']['A'] = [self.cell]
        self.spec['arms'] = {a: value for a, value in self.spec['arms'].items()
                            if a in {'baseline', 'incumbent', 'incremental-many'}}
        for cells in self.spec['panels'].values():
            for cell in cells:
                cell['comparators'] = []
        self.spec['arms']['incumbent']['sha256'] = h('candidate')
        self.spec['arms']['incremental-many']['sha256'] = h('candidate')
        self.meta.update(absorbed_terms=3, categorical_intercepts=True, body_sha256=self.cell['body_sha256'])
        self.plugins = {a: self.baseline if a == 'baseline' else self.plugin for a in self.spec['arms']}
        self.evidence.update(manifest_sha256=routing.identity(self.spec), body_sha256=self.cell['body_sha256'], arms=copy.deepcopy(self.spec['arms']))
        self.witness.write_text(json.dumps(self.evidence))

    def test_real_resource_adapter_accepts_uninjected_incumbent(self):
        self.assertIsNone(routing_run.comparison_policy(self.spec, 'incumbent', self.meta))
        native_result = run_tests.result
        auto_result = run_tests.PortfolioAutoScope.raw('map-no-lsmr')
        with patch.object(run_tests, 'result', side_effect=lambda arm:
                          copy.deepcopy(auto_result) if arm == 'auto' else native_result(arm)):
            value, _ = self.run_panel(prerequisites_only=True)
        incumbent = next(r for r in value['prerequisites'] if r['arm'] == 'incumbent')
        self.assertEqual(incumbent['status'], 'passed', value['prerequisites'])
        self.assertEqual(incumbent['resource_checks']['setup_caps'], 'not_applicable')
        self.assertTrue(routing.verified_record(incumbent, self.spec))
        self.assertEqual(value['status'], 'complete')
        self.assertEqual(set(value['qualified_wrappers']), {self.cell['id']})

    def test_explicit_certificate_failure_is_retained_diagnostic_not_gate(self):
        self.spec['arms']['lsmr'] = {'sha256': h('candidate'), 'role': 'optional_comparator'}
        self.cell['comparators'] = ['lsmr']
        self.plugins['lsmr'] = self.plugin
        self.evidence.update(manifest_sha256=routing.identity(self.spec), arms=copy.deepcopy(self.spec['arms']))
        self.witness.write_text(json.dumps(self.evidence))
        auto = run_tests.PortfolioAutoScope.raw('map-no-lsmr')
        failed = run_tests.result('lsmr')
        failed['matrices']['RCERT']['bits'][-1] = run_tests.bits(0.)
        with patch.object(run_tests, 'result', side_effect=lambda arm: copy.deepcopy(auto if arm == 'auto' else failed)):
            value, _ = self.run_panel(prerequisites_only=True)
        row = next(r for r in value['prerequisites'] if r['arm'] == 'lsmr')
        self.assertTrue(routing.diagnostic_invalid(row, self.spec))
        self.assertEqual(row['failure']['kind'], 'comparator_science_invalid')
        self.assertTrue(row['failure_artifacts'])
        self.assertEqual(set(value['qualified_wrappers']), {self.cell['id']})
        self.assertTrue(all(r['status'] == 'passed' for r in value['prerequisites'] if r['arm'] != 'lsmr'))
        self.assertFalse(routing_run.native_comparator_scientific_mismatch('unsupported RSOLVER layout'))
        self.assertFalse(routing_run.native_comparator_scientific_mismatch('missing/invalid monotonic native timing'))

    def test_public_and_hidden_outside_scope_require_exact_incumbent(self):
        spec = copy.deepcopy(self.spec)
        for arm in ('public-auto', 'selected-hidden'):
            spec['arms'][arm] = {'effective_policy': routing.policy('incremental-many')}
        raw = run_tests.PortfolioAutoScope.raw('lsmr')
        changed = copy.deepcopy(raw)
        changed['matrices']['RSOLVER']['bits'][2] = run_tests.bits(99.)
        left, right = self.root / 'left.columns', self.root / 'right.columns'
        left.write_bytes(b'exact'); right.write_bytes(b'exact')
        for arm in ('incremental-many', 'public-auto', 'selected-hidden'):
            routing_run.incremental_exact_outside(spec, arm, {'absorbed_terms': 2, 'categorical_intercepts': True},
                                                 raw, raw, left, right)
            with self.subTest(arm=arm), self.assertRaisesRegex(ValueError, 'outside-family'):
                routing_run.incremental_exact_outside(spec, arm, {'absorbed_terms': 2, 'categorical_intercepts': True},
                                                     raw, changed, left, right)
        self.assertIsNone(routing_run.comparison_policy(spec, 'incremental-many',
                                                       {'absorbed_terms': 2, 'categorical_intercepts': True}))


class IncrementalExecutionTests(unittest.TestCase):
    node = parallel_tests.PartitionParallelTests.node
    six_nodes = parallel_tests.PartitionParallelTests.six_nodes

    def setUp(self):
        parallel_tests.PartitionParallelTests.setUp(self)
        self.spec = incremental_spec()
        test = discovery.reference(__file__)
        self.frozen[test['path']] = test['sha256']
        self.parent.update(kind=discovery.INCREMENTAL_KIND + '_plan', tournament=routing.seal(self.spec),
                           plugins={a: 'unused' for a in self.spec['arms']}, frozen_files=dict(self.frozen))
        self.parent_ref = save_json(self.root / 'incremental-parent.json', routing.seal(self.parent))
        self.frozen[self.parent_ref['path']] = self.parent_ref['sha256']
        self.plan.update(design=copy.deepcopy(parallel.INCREMENTAL_DESIGN), parent=routing.seal(self.parent),
                         parent_plan=self.parent_ref, parent_sha256=routing.identity(self.parent),
                         partitions=discovery.partitions(self.spec))

    def test_closed_freeze_partition_views_and_all_six_receipts(self):
        self.assertEqual(parallel.validate_plan(self.plan), self.spec)
        check = self.root / 'checks.txt'; check.write_text('passed')
        config = {'schema': 3, 'discovery_plan': self.parent_ref['path'],
                  'output_root': str(self.root / 'incremental-freeze'),
                  'receipt_generator': parallel.isolation_host.__file__, 'source_checks': [str(check)],
                  'scheduler_query_policy': discovery.SCHEDULER_QUERY_POLICY['kind']}
        with patch.object(routing, 'scratch', side_effect=lambda p: p.resolve()):
            frozen = parallel.freeze(config)
        self.assertEqual(frozen['design'], parallel.INCREMENTAL_DESIGN)
        self.assertEqual(parallel.metadata(frozen)['spec'], self.spec)
        with patch.object(discovery, 'isolation'):
            reports = self.six_nodes()
            result = parallel.aggregate(self.plan, None, reports)
        self.assertEqual(result['kind'], discovery.INCREMENTAL_KIND + '_shortlist')
        self.assertEqual(result['shortlist'], ['incumbent', 'incremental-many'])
        self.assertEqual(result['ranking'][0]['arm'], 'incumbent')
        self.assertEqual(len(result['collection']['attempts']), 6)
        with patch.object(discovery, 'score') as score:
            with self.assertRaises(ValueError):
                parallel.aggregate(self.plan, None, reports[:5])
            score.assert_not_called()

    def test_optional_cross_host_and_worker_signatures_do_not_veto(self):
        with patch.object(discovery, 'isolation'):
            for n in (1, 2, 3):
                for shard in (1, 2):
                    view, collection, proof = self.node(n, shard=shard)
                    for ref in (proof['records'], collection['rounds'][0]):
                        value = routing.read_json(Path(ref['path']))
                        for row in [*value['prerequisites'], *value['records']]:
                            if routing.arm_role(self.spec, row['arm']) == 'optional_comparator':
                                row['solver_rows'][0][1] = n
                                row['model_discrete_sha256'] = h('diagnostic-' + row['cell'] + str(n))
                        saved = save_json(Path(ref['path']), value)
                        ref.update(saved)
                    root = Path(view['output_root'])
                    proof_ref = save_json(root / 'prerequisites/prerequisites.json', routing.seal(proof))
                    collection.update(prerequisites=proof_ref, prerequisites_sha256=routing.identity(proof))
                    save_json(root / 'collection/collection.json', routing.seal(collection))
            reports = [parallel.inspect_attempt(self.plan, None, n, 1, shard)
                       for n in (1, 2, 3) for shard in (1, 2)]
            result = parallel.aggregate(self.plan, None, reports)
        self.assertTrue(all(row['pass'] for row in result['ranking']))
        self.assertTrue(result['diagnostic_invalidity'])
        self.assertTrue(all(not comparison['runtime_identified']
                            for row in result['ranking'] for comparison in row['comparisons']
                            if not comparison['protection']))

    def terminal_inputs(self, result, outcome='retained'):
        evidence = self.root / 'independent-review.txt'
        evidence.write_text('Fixture review of bound raw discovery evidence; no passing claim for failed prerequisites.')
        review = {'kind': discovery.INCREMENTAL_KIND + '_terminal_review', 'status': 'passed',
                  'outcome': outcome, 'manifest_sha256': routing.identity(self.spec),
                  'plan_sha256': routing.identity(self.plan), 'result_sha256': routing.identity(result),
                  'all_allocations_terminal': True, 'outstanding_jobs': [],
                  'evidence': [discovery.reference(evidence)]}
        return {'manifest': save_json(self.root / 'terminal-manifest.json', routing.seal(self.spec)),
                'plan': save_json(self.root / 'terminal-plan.json', routing.seal(self.plan)),
                'result': save_json(self.root / 'terminal-result.json', routing.seal(result)),
                'review': save_json(self.root / 'terminal-review.json', review)}

    def test_complete_rejected_a_replays_before_retained_terminal(self):
        with patch.object(discovery, 'isolation'):
            for n in (1, 2, 3):
                for shard in (1, 2):
                    view, collection, _ = self.node(n, shard=shard)
                    bound = collection['rounds'][0]
                    timed = routing.read_json(Path(bound['path']))
                    for row in timed['records']:
                        if row['arm'] == 'incremental-many':
                            row['seconds'] = 1.2
                    bound.update(save_json(Path(bound['path']), timed))
                    save_json(Path(view['output_root']) / 'collection/collection.json', routing.seal(collection))
            reports = [parallel.inspect_attempt(self.plan, None, n, 1, shard)
                       for n in (1, 2, 3) for shard in (1, 2)]
            result = parallel.aggregate(self.plan, None, reports)
            self.assertEqual(result['shortlist'], ['incumbent'])
            inputs = self.terminal_inputs(result)
            self.assertEqual(discovery.terminal_basis(inputs), (self.spec, result))
            review = routing.read_json(Path(inputs['review']['path']))
            for mutation in ({'outcome': 'accepted'}, {'outstanding_jobs': ['still-running']},
                             {'all_allocations_terminal': False}):
                changed = {**review, **mutation}
                bad = {**inputs, 'review': save_json(self.root / 'changed-review.json', changed)}
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    discovery.terminal_basis(bad)

    def test_incomplete_a_retains_incumbent_without_passing_claim_or_omitting_allocations(self):
        view, collection, _ = self.node(1, shard=1)
        collection.update(status='incomplete', error='candidate scientific prerequisite failed')
        ref = save_json(Path(view['output_root']) / 'collection/collection.json', routing.seal(collection))
        result = discovery.failed_campaign(self.plan, [ref])
        self.assertEqual(result['status'], 'failed')
        self.assertFalse(result['promotion'])
        self.assertEqual(discovery.terminal_basis(self.terminal_inputs(result)), (self.spec, result))
        with self.assertRaises(ValueError):
            discovery.terminal_basis(self.terminal_inputs(result, outcome='accepted'))
        extra = Path(self.plan['output_root']) / 'round-2/shard-1/attempt-1'
        extra.mkdir(parents=True)
        with self.assertRaisesRegex(ValueError, 'every started allocation'):
            discovery.failed_campaign(self.plan, [ref])

    def test_passing_a_challenger_cannot_be_discarded_as_retained_terminal(self):
        with patch.object(discovery, 'isolation'):
            result = parallel.aggregate(self.plan, None, self.six_nodes())
            self.assertIn('incremental-many', result['shortlist'])
            with self.assertRaisesRegex(ValueError, 'rederived rejected challenger'):
                discovery.terminal_basis(self.terminal_inputs(result))


if __name__ == '__main__':
    unittest.main()
