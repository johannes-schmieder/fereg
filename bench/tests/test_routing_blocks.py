"""Synthetic metadata and proof boundaries; no native work or holdout bodies."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import routing
import routing_blocks as blocks
import routing_discovery as discovery
import routing_run as runner
from test_routing import h, manifest


class BlockContext(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.serial = 0
        self.spec = manifest((1, 2))
        for stage in ['A', 'B']:
            cell = self.spec['panels'][stage][0]
            self.spec['panels'][stage] = [{**cell, 'id': f'{stage}-{i}',
                'case_sha256': h(f'{stage}-case-{i}'), 'body_sha256': h(f'{stage}-body-{i}')}
                for i in range(42)]
        self.spec['candidate_source_identity'] = {'production_tokens_sha256': h('production'),
                                                 'plugin_sha256': h('candidate')}
        self.plan = {'kind': discovery.KIND + '_plan', 'design': discovery.DESIGN,
                     'tournament': routing.seal(self.spec)}
        ranking = ['h8-k0', 'baseline', 'h16-k0', 'h32-k0']
        ranking += [arm for arm in self.spec['candidates'] if arm not in ranking]
        self.summary = {'kind': discovery.KIND + '_shortlist', 'status': 'discovery_only',
            'promotion': False, 'plan_sha256': routing.identity(self.plan),
            'collection_sha256': h('collection'), 'isolation_sha256': h('isolation'),
            'shortlist': ranking[:4], 'ranking': [{'arm': arm} for arm in ranking]}
        remote = self.write({'status': 'passed', 'result_sha256': routing.identity(self.summary),
                             'plan_sha256': routing.identity(self.plan)})
        self.review = {'kind': blocks.KIND + '_discovery_review', 'status': 'passed',
            'plan_sha256': routing.identity(self.plan), 'result_sha256': routing.identity(self.summary),
            'collection_sha256': h('collection'), 'isolation_sha256': h('isolation'),
            'production_tokens_sha256': h('production'), 'remote_validation': remote,
            'raw_evidence': [self.write({'retained': 'scheduler and complete observations'})]}
        pilot = {'kind': blocks.KIND + '_pilot_review', 'status': 'passed',
                 'recommended_attempts': {'B': 12, 'C': 24}, 'evidence': [self.write({'pilot': 'observations'})]}
        self.inputs = {'manifest': self.write(routing.seal(self.spec)),
            'discovery_plan': self.write(routing.seal(self.plan)),
            'discovery_result': self.write(routing.seal(self.summary)),
            'discovery_review': self.write(self.review), 'pilot_review': self.write(pilot)}
        self.design = {'attempts': 12, 'warmups': 1, 'seed': 17, 'timeout_s': 120.,
            'budget_s': 3600., 'session_id': 'prospective-test', 'analysis_unit': 'block',
            'alpha': {'numerator': 1, 'denominator': 40}}
        self.context = blocks.declare(self.inputs, self.design)
        self.context_ref = self.write(routing.seal(self.context))

    def write(self, value):
        self.serial += 1
        path = self.root / f'{self.serial}.json'
        path.write_text(json.dumps(value))
        return blocks.reference(path)

    def qualified(self):
        rows, prerequisites, wrappers, prepared = [], [], {}, {}
        arms = set()
        for cell in self.context['cells']:
            cid = cell['id']
            wrapper = self.write({'synthetic_wrapper': cid})
            wrappers[cid] = {'path': Path(wrapper['path']).name, 'sha256': wrapper['sha256']}
            for arm in blocks.required_arms(self.context, cell):
                arms.add(arm)
                row = {'cell': cid, 'arm': arm, 'status': 'passed', 'repetition': 0,
                    'certificate_pass': True, 'science_pass': True,
                    'science_sha256': h(cid + arm), 'witness_sha256': wrapper['sha256'],
                    'prepared_sha256': h('prepared-' + cid + arm),
                    'resource_checks': {key: 'passed' for key in ['iteration_caps', 'memory_caps', 'setup_caps']}}
                rows.append(row)
                prerequisites.append(dict(row, repetition=-1))
                prepared[cid + '/' + arm] = row['prepared_sha256']
        result = {'platform': 'Darwin-test', 'purpose': 'scientific_prerequisites', 'status': 'complete',
            'stage': 'B', 'selection_sha256': routing.identity(self.context),
            'qualification_context': self.context_ref, 'manifest_sha256': routing.identity(self.spec),
            'harness_sha256': self.spec['harness_sha256'],
            'artifacts': {arm: self.spec['arms'][arm]['sha256'] for arm in arms},
            'cases': {cell['id']: cell['case_sha256'] for cell in self.context['cells']},
            'tool_sha256': {p.name: v for p, v in runner.tool_identities().items()},
            'warmups': 0, 'repetitions': 1, 'timeout_s': 120., 'budget_s': 14400.,
            'qualified_wrappers': wrappers, 'prerequisites': prerequisites,
            'records': rows, 'prepared_sha256': prepared}
        return result

    def test_declaration_uses_all_original_B_metadata_without_body_access(self):
        self.assertEqual(self.context['cells'], self.spec['panels']['B'])
        self.assertEqual(self.context['candidates'], self.summary['shortlist'])
        self.assertNotIn('selected', self.context)
        with patch.object(blocks.platform, 'system', return_value='Darwin'):
            value = blocks.prerequisite_context(self.spec, 'B', None, self.context_ref)
        self.assertEqual(value['selection_sha256'], routing.identity(self.context))
        self.assertEqual(value['candidates'], self.summary['shortlist'])

    def test_changed_coverage_policy_source_and_unfrozen_budget_rejected(self):
        for change in ['body', 'policy', 'source', 'cell']:
            spec = copy.deepcopy(self.spec)
            if change == 'body': spec['panels']['B'][0]['body_sha256'] = h('changed')
            elif change == 'policy': spec['protections'].append('h8-k0')
            elif change == 'source': spec['candidate_source_identity']['production_tokens_sha256'] = h('changed')
            else: spec['panels']['B'].pop()
            with self.subTest(change=change), self.assertRaises(ValueError):
                blocks.declare({**self.inputs, 'manifest': self.write(routing.seal(spec))}, self.design)
        for field, value in [('attempts', 14), ('budget_s', None), ('alpha', {'numerator': 1, 'denominator': 20})]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                blocks.declare(self.inputs, {**self.design, field: value})

    def test_raw_discovery_and_actual_remote_validation_required(self):
        for change in ['raw', 'validation', 'selected']:
            review = copy.deepcopy(self.review)
            if change == 'raw': review['raw_evidence'] = []
            elif change == 'validation': review['remote_validation'] = self.write({'status': 'failed'})
            else: review['result_sha256'] = h('another shortlist')
            with self.subTest(change=change), self.assertRaises(ValueError):
                blocks.declare({**self.inputs, 'discovery_review': self.write(review)}, self.design)

    def test_parallel_handoff_validates_embedded_parent_before_body_access(self):
        import routing_parallel as parallel
        parent = {**self.plan, 'library_limits': discovery.LIBRARY_LIMITS,
                  'producer_tools': {'producer.py': h('producer')}, 'frozen_files': {}}
        refs = {name: self.write({'bound': name})
                for name in ['parent_plan', 'proof', 'receipt_generator']}
        plan = {'kind': parallel.KIND + '_plan', 'schema': 1, 'design': parallel.DESIGN,
                'parent': routing.seal(parent), 'parent_sha256': routing.identity(parent),
                **refs, 'proof_sha256': h('proof'), 'output_root': str(self.root / 'parallel'),
                'producer_tools': parent['producer_tools'],
                'frozen_files': {ref['path']: ref['sha256'] for ref in refs.values()}}
        summary = {**self.summary, 'plan_sha256': routing.identity(plan)}
        validation = self.write({'status': 'passed', 'result_sha256': routing.identity(summary),
                                 'plan_sha256': routing.identity(plan)})
        review = {**self.review, 'plan_sha256': routing.identity(plan),
                  'result_sha256': routing.identity(summary), 'remote_validation': validation}
        inputs = {**self.inputs, 'discovery_plan': self.write(routing.seal(plan)),
                  'discovery_result': self.write(routing.seal(summary)),
                  'discovery_review': self.write(review)}
        with patch.object(runner, 'inspect_case') as inspect:
            context = blocks.declare(inputs, self.design)
            inspect.assert_not_called()
        names = {Path(ref['path']).name for ref in
                 blocks.context_bindings(self.write(routing.seal(context)), context)}
        self.assertIn('routing_parallel.py', names)
        forged = {**plan, 'parent_sha256': h('different parent')}
        with self.assertRaises(ValueError):
            blocks.declare({**inputs, 'discovery_plan': self.write(routing.seal(forged))}, self.design)

    def test_C_legacy_previous_and_linux_rejected_before_case_access(self):
        with patch.object(blocks.platform, 'system', return_value='Darwin'):
            for stage, previous in [('C', None), ('B', {'stage': 'A'})]:
                with self.subTest(stage=stage), self.assertRaises(ValueError):
                    blocks.prerequisite_context(self.spec, stage, previous, self.context_ref)
        with patch.object(blocks.platform, 'system', return_value='Linux'), self.assertRaises(ValueError):
            blocks.prerequisite_context(self.spec, 'B', None, self.context_ref)
        with patch.object(runner, 'inspect_case') as inspect:
            with self.assertRaisesRegex(ValueError, 'scientific-prerequisites-only'):
                runner.run(self.spec, 'B', None, Path('unused'), {}, {}, {}, self.root / 'out',
                           qualification_context=self.context_ref)
            inspect.assert_not_called()

    def test_fresh_Mac_stage_tool_and_full_wrapper_coverage_required(self):
        records = self.qualified()
        blocks.qualification(self.context_ref, self.write(records))
        for key, value in [('platform', 'Linux'), ('stage', 'A'), ('selection_sha256', None),
                           ('qualification_context', None), ('tool_sha256', {}), ('qualified_wrappers', {})]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                blocks.qualification(self.context_ref, self.write({**records, key: value}))

    def test_all_candidate_reference_edges_and_identity_protection_retained(self):
        coverage = blocks.comparison_coverage(self.context_ref, self.write(self.qualified()))
        self.assertEqual(len(coverage['comparisons']), 42 * 4 * 2)
        self.assertEqual(sum(row['status'] == 'identity' for row in coverage['comparisons']), 42)
        self.assertEqual({row['reference_role'] for row in coverage['comparisons']}, {'protected', 'internal'})
        self.assertFalse(coverage['excluded_candidates'])

    def test_optional_Mac_failure_is_carried_and_required_failure_blocks(self):
        for arm in ['lsmr', 'baseline']:
            records = self.qualified()
            before = next(row for row in records['prerequisites'] if row['cell'] == 'B-0' and row['arm'] == arm)
            before.update(status='unavailable', failure={'kind': 'timeout', 'dispatched': True,
                'call_limit_s': 120., 'elapsed_s': 120.1, 'log_sha256': h('timeout'),
                'prepared_sha256': before['prepared_sha256']})
            after = next(row for row in records['records'] if row['cell'] == 'B-0' and row['arm'] == arm)
            after.update(status='not_run', blocked_by_sha256=routing.observation_identity(before))
            ref = self.write(records)
            if arm == 'baseline':
                with self.assertRaises(ValueError): blocks.comparison_coverage(self.context_ref, ref)
            else:
                coverage = blocks.comparison_coverage(self.context_ref, ref)
                self.assertEqual(sum(row['status'] == 'unavailable' for row in coverage['comparisons']), 4)

    def timing_config(self):
        records = self.qualified()
        qualified = self.write(records)
        def binary(name, value):
            path = self.root / name
            path.write_bytes(value.encode())
            return blocks.reference(path)
        harness = binary('harness', 'harness')
        plugins = {name: binary(name, name) for name in ['candidate', 'baseline']}
        cells = {}
        for cell in self.context['cells']:
            cid = cell['id']
            arms = {}
            for arm in blocks.required_arms(self.context, cell):
                arms[arm] = {'plugin': plugins['baseline' if arm == 'baseline' else 'candidate'],
                    'input': binary(cid + arm + '.case', 'prepared-' + cid + arm)}
            cells[cid] = {'source': binary(cid + '.case', cid.replace('B-', 'B-case-')), 'arms': arms}
        config = {'context': self.context_ref, 'qualification': qualified, 'harness': harness,
                  'monitor': None, 'cells': cells}
        return config

    def test_timing_spec_derives_complete_mirrored_edges_and_packets(self):
        config = self.timing_config()
        with patch.object(runner, 'evidence', return_value=({}, {})):
            spec = blocks.timing_spec(config)
        self.assertEqual(len(spec['cells']), 42)
        self.assertEqual(sum(len(cell['comparisons']) for cell in spec['cells']), 42 * 7)
        self.assertTrue(all(cell['validation']['kind'] == 'routing_blocks' for cell in spec['cells']))
        self.assertEqual(spec['design']['attempts'], 12)
        with self.assertRaisesRegex(ValueError, 'complete original B'):
            blocks.timing_spec({**config, 'cells': {k: v for k, v in config['cells'].items() if k != 'B-0'}})
        packet = spec['cells'][0]['validation']
        with self.assertRaisesRegex(ValueError, 'fresh Mac qualification'):
            blocks.science_context({**packet, 'witness': self.write({'stale': 'other host'})})

    def test_context_dependency_inventory_binds_discovery_and_review_closure(self):
        names = {Path(bound['path']).name for bound in blocks.context_bindings(self.context_ref, self.context)}
        self.assertIn('routing_blocks.py', names)
        self.assertIn('routing_discovery.py', names)
        self.assertIn(Path(self.review['remote_validation']['path']).name, names)
        changed = copy.deepcopy(self.context)
        changed['discovery_validator_sha256'] = h('changed')
        with self.assertRaises(ValueError): blocks.load_context(self.write(routing.seal(changed)))

    def test_new_producer_hook_publishes_context_bound_B_science_without_legacy_selection(self):
        from test_routing_run import RoutingProducer
        fixture = RoutingProducer()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.spec['panels']['A'][0]['case_sha256'] = h('A')
        fixture.cell = fixture.spec['panels']['B'][0]
        fixture.cell['case_sha256'] = blocks.digest(fixture.source)
        fixture.meta['body_sha256'] = fixture.cell['body_sha256']
        fixture.evidence.update(cell_id=fixture.cell['id'],
            case_sha256=fixture.cell['case_sha256'], body_sha256=fixture.cell['body_sha256'],
            manifest_sha256=routing.identity(fixture.spec))
        fixture.witness.write_text(json.dumps(fixture.evidence))
        original_run = runner.run
        def with_context(*args, **kwargs):
            return original_run(*args, **kwargs, qualification_context=self.context_ref)
        resolved = {'candidates': fixture.spec['candidates'],
                    'selection_sha256': routing.identity(self.context),
                    'bindings': blocks.context_bindings(self.context_ref, self.context)}
        with patch.object(blocks, 'prerequisite_context', return_value=resolved), \
             patch.object(routing, 'required_candidates', side_effect=AssertionError('legacy selection called')), \
             patch.object(runner, 'run', side_effect=with_context):
            output, calls = fixture.run_panel(prerequisites_only=True, stage='B')
        self.assertEqual(output['status'], 'complete')
        self.assertEqual(output['qualification_context'], self.context_ref)
        self.assertEqual(len(calls), 2 * len(fixture.spec['arms']))
        wrapper = output['qualified_wrappers'][fixture.cell['id']]
        value = routing.read_json(fixture.root / 'output' / wrapper['path'])
        self.assertEqual(value['stage'], 'B')
        self.assertEqual(value['selection_sha256'], routing.identity(self.context))

    def decision_fixture(self):
        config = self.timing_config()
        with patch.object(runner, 'evidence', return_value=({}, {})):
            spec = blocks.timing_spec(config)
        analysis = {'missing_monitor_evidence': False, 'scientific_or_execution_failure': False,
                    'summary': [{'cell_id': cell['id'], 'comparison_id': pair['id'],
                        'candidate_arm': pair['candidate'], 'reference_arm': pair['reference'],
                        'reference_role': pair['reference_role'], 'conditional_nonregression_pass': True,
                        'attempted_slots': 12, 'excess_median_interval': {'upper': -.01},
                        'normalized_regret_median_interval': {'upper': .5}}
                        for cell in spec['cells'] for pair in cell['comparisons']]}
        bound = self.write({'synthetic': 'timing bindings'})
        return config, spec, analysis, bound

    def decide_synthetic(self, config, spec, analysis, bound):
        import time_blocks
        with patch.object(runner, 'evidence', return_value=({}, {})), \
             patch.object(time_blocks, 'load_manifest', return_value=({'spec': spec}, {})), \
             patch.object(time_blocks, 'analyze', return_value=analysis):
            return routing.unseal(blocks.decide(config, bound, bound, bound))

    def test_B_decision_uses_all_edges_and_conservative_ties_not_legacy_selector(self):
        config, spec, analysis, bound = self.decision_fixture()
        with patch.object(routing, 'evaluate', side_effect=AssertionError('legacy selector called')):
            result = self.decide_synthetic(config, spec, analysis, bound)
        self.assertEqual(result['finalist'], 'baseline')
        self.assertFalse(result['promotion'])
        self.assertNotIn('stage', result)
        self.assertNotIn('selected', result)
        for row in analysis['summary']:
            if row['candidate_arm'] == 'h8-k0' and row['reference_role'] == 'internal':
                row['normalized_regret_median_interval']['upper'] = .1
        self.assertEqual(self.decide_synthetic(config, spec, analysis, bound)['finalist'], 'h8-k0')

    def test_B_missing_inconclusive_unmonitored_and_changed_coverage_cannot_select(self):
        config, spec, analysis, bound = self.decision_fixture()
        for change in ['missing', 'inconclusive', 'monitor', 'science']:
            bad = copy.deepcopy(analysis)
            if change == 'missing': bad['summary'] = []
            elif change == 'inconclusive':
                for row in bad['summary']: row['excess_median_interval']['upper'] = None
            elif change == 'monitor': bad['missing_monitor_evidence'] = True
            else: bad['scientific_or_execution_failure'] = True
            with self.subTest(change=change):
                self.assertIsNone(self.decide_synthetic(config, spec, bad, bound)['finalist'])
        bad_spec = copy.deepcopy(spec)
        bad_spec['cells'][0]['comparisons'].pop()
        with self.assertRaisesRegex(ValueError, 'complete prospective B'):
            self.decide_synthetic(config, bad_spec, analysis, bound)
        duplicate = copy.deepcopy(analysis)
        duplicate['summary'].append(copy.deepcopy(duplicate['summary'][0]))
        with self.assertRaises(ValueError): self.decide_synthetic(config, spec, duplicate, bound)


if __name__ == '__main__':
    unittest.main()
