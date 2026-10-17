"""Incremental Mac protection, objective and frozen-evidence boundaries."""
import copy
import unittest
from unittest.mock import patch

import routing
import routing_blocks as blocks
import routing_block_deployment as deployment
import routing_discovery as discovery
import routing_parallel as parallel
from test_routing import h
import test_routing_blocks


class IncrementalMac(unittest.TestCase):
    def setUp(self):
        self.fixture = test_routing_blocks.BlockContext()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.write = self.fixture.write
        spec, plan, summary, review, inputs = self.fixture.portfolio_inputs(3)
        spec = copy.deepcopy(spec)
        old = routing.portfolio_candidate(spec)
        candidate = 'incremental-many'
        spec.update(schema=3, experiment_kind='incremental', candidates=['incumbent', candidate],
            protections=['baseline', 'incumbent'], failure_policy=copy.deepcopy(routing.INCREMENTAL_FAILURE_POLICY),
            incremental={'family': 'many', 'sequence': 1, 'original_revision': 'f35f3a4d',
                         'incumbent_revision': 'a' * 40})
        spec['arms'].pop(old)
        spec['arms']['incumbent'] = {'sha256': h('accepted'), 'policy': None, 'role': 'required_reference'}
        spec['arms'][candidate] = {'sha256': h('candidate'), 'policy': routing.policy(candidate), 'role': 'candidate'}
        for stage in ['B', 'C']:
            spec['panels'][stage][0].update(categorical_intercepts=True, absorbed_terms=3)
        parent = routing.unseal(plan['parent'])
        parent.update(kind=discovery.INCREMENTAL_KIND + '_plan', tournament=routing.seal(spec))
        plan.update(parent=routing.seal(parent), parent_sha256=routing.identity(parent),
                    parent_plan=self.write(routing.seal(parent)), design=copy.deepcopy(parallel.INCREMENTAL_DESIGN),
                    partitions=discovery.partitions(spec))
        plan['frozen_files'][plan['parent_plan']['path']] = plan['parent_plan']['sha256']
        summary.update(kind=discovery.INCREMENTAL_KIND + '_shortlist', experiment_kind='incremental',
                       plan_sha256=routing.identity(plan), ranking=[
                           {'arm': arm, 'pass': True, 'data_valid': True} for arm in spec['candidates']])
        summary.update(parallel.incremental_screen(summary['ranking'], spec))
        summary['collection']['plan_sha256'] = routing.identity(plan)
        for report in summary['collection']['attempts']:
            report.update(parallel_plan_sha256=routing.identity(plan),
                          partition_sha256=routing.identity(plan['partitions'][report['shard']-1]))
        summary['collection_sha256'] = routing.identity(summary['collection'])
        shared = {'status': 'passed', 'plan_sha256': routing.identity(plan),
                  'result_sha256': routing.identity(summary), 'collection_sha256': summary['collection_sha256'],
                  'isolation_sha256': summary['isolation_sha256']}
        review.update(shared, remote_validation=self.write({**shared, 'recomputed_result': summary}),
                      independent_raw_review=self.write({**shared, 'evidence': review['raw_evidence'],
                        'prerequisites': [r['prerequisites'] for r in summary['collection']['attempts']]}))
        inputs.update(manifest=self.write(routing.seal(spec)), discovery_plan=self.write(routing.seal(plan)),
                      discovery_result=self.write(routing.seal(summary)), discovery_review=self.write(review))
        self.spec, self.plan, self.summary, self.review, self.inputs = spec, plan, summary, review, inputs
        self.design = {**self.fixture.design, 'budget_s': 14400}
        self.context = blocks.declare(inputs, self.design)
        self.bound = self.write(routing.seal(self.context))

    def test_closed_handoff_and_frozen_budgets(self):
        self.assertEqual(blocks.load_context(self.bound), (self.context, self.spec))
        self.assertEqual(self.context['protections'], ['baseline', 'incumbent'])
        for budget in (3600, 14401):
            with self.assertRaises(ValueError):
                blocks.declare(self.inputs, {**self.design, 'budget_s': budget})
        c = {**self.design, 'attempts': 24, 'budget_s': 28800, 'seed': 19, 'session_id': 'C'}
        declared = deployment.declare(self.bound, c)
        self.assertEqual(declared['contract']['allowed_production_paths'], ['crates/fereg-core/src/fe/session.rs'])
        for mutation in ({'seed': 17}, {'session_id': self.design['session_id']}, {'budget_s': 14400}):
            with self.assertRaises(ValueError):
                deployment.declare(self.bound, {**c, **mutation})
        bad = copy.deepcopy(self.context)
        bad['protections'] = ['baseline']
        with self.assertRaises(ValueError):
            blocks.load_context(self.write(routing.seal(bad)))

    def decision(self, *, ratio=.9, protected_upper=-.001, omit=False):
        import time_blocks
        ratios = ratio if isinstance(ratio, tuple) else (ratio,)
        prototype = next(c for c in self.context['cells'] if routing.incremental_eligible(c, 'many'))
        cells = [{**prototype, 'id': f'objective-{i}'} for i in range(len(ratios))]
        context = {**self.context, 'cells': cells}
        comparisons, units, rows = [], [], []
        for cell, ratio in zip(cells, ratios):
            edges = [{'cell': cell['id'], 'candidate': arm, 'reference': ref,
                      'reference_role': 'protected' if ref in context['protections'] else 'internal',
                      'status': 'identity' if arm == ref else 'diagnostic' if ref == 'lsmr' else 'timed'}
                     for arm in context['candidates'] for ref in ['baseline', 'incumbent', 'lsmr']]
            comparisons.extend(edges)
            pairs = [{**{k: edge[k] for k in ['candidate', 'reference', 'reference_role']},
                      'id': f"{cell['id']}-p{i}"} for i, edge in enumerate(edges) if edge['status'] == 'timed']
            units.append({'id': cell['id'], 'comparisons': pairs})
            rows.extend({'cell_id': cell['id'], 'comparison_id': pair['id'],
                         'candidate_arm': pair['candidate'], 'reference_arm': pair['reference'],
                         'reference_role': pair['reference_role'], 'attempted_slots': 12,
                         'conditional_nonregression_pass': protected_upper <= 0,
                         'excess_median_interval': {'upper': protected_upper},
                         'normalized_regret_median_interval': {'upper': .5},
                         'ratio_median_interval_descriptive': {'median': ratio if pair['candidate'] != 'incumbent' else 1.0}}
                        for pair in pairs)
        expected = {'cells': units}
        analysis = {'missing_monitor_evidence': False, 'scientific_or_execution_failure': False, 'summary': rows}
        if omit:
            analysis['summary'] = [r for r in analysis['summary'] if r['reference_arm'] != 'incumbent']
        coverage = {'candidates': context['candidates'], 'comparisons': comparisons, 'excluded_candidates': []}
        with patch.object(blocks, 'load_context', return_value=(context, self.spec)), \
             patch.object(blocks, 'timing_spec', return_value=expected), \
             patch.object(blocks, 'comparison_coverage', return_value=coverage), \
             patch.object(time_blocks, 'load_manifest', return_value=({'spec': expected}, {})), \
             patch.object(time_blocks, 'analyze', return_value=analysis):
            return routing.unseal(blocks.decide({'context': self.bound, 'qualification': self.bound},
                                               self.bound, self.bound, self.bound))

    def test_family_objective_incumbent_ties_and_all_protected_edges(self):
        self.assertEqual(self.decision()['finalist'], 'incremental-many')
        self.assertEqual(self.decision(ratio=1.)['finalist'], 'incumbent')
        self.assertEqual(self.decision(ratio=1.01)['finalist'], 'incumbent')
        self.assertEqual(self.decision(ratio=(.5, 2.))['finalist'], 'incumbent')
        self.assertEqual(self.decision(ratio=(.5, 1.5))['finalist'], 'incremental-many')
        self.assertIsNone(self.decision(protected_upper=.0001)['finalist'])
        self.assertEqual(self.decision(omit=True)['finalist'], 'incumbent')
        row = self.decision()['ranking'][0]
        self.assertEqual(row['affected_family_geometric_mean_ratio'], .9)
        self.assertTrue(all(edge['pass'] is None for edge in row['comparisons'] if edge['reference_role'] == 'internal'))

    def test_executable_timing_frontier_excludes_explicit_diagnostic_arms(self):
        cell = self.context['cells'][0]
        context = {**self.context, 'cells': [cell]}
        # The production coverage resolver consumes producer outcomes; only the
        # artifact execution itself is replaced here by synthetic qualification.
        outcomes = {arm: {'status': 'passed'} for arm in blocks.required_arms(context, cell)}
        records = {'prerequisites': [], 'records': []}
        with patch.object(blocks, 'qualification', return_value=(context, self.spec, records)), \
             patch.object(blocks.runner, 'prerequisite_outcomes', return_value=outcomes):
            coverage = blocks.comparison_coverage(self.bound, self.bound)
        self.assertTrue(all(edge['status'] == 'diagnostic' for edge in coverage['comparisons']
                            if edge['reference_role'] == 'internal'))
        explicit = cell['comparators'][0]
        outcomes[explicit]['status'] = 'invalid'
        with patch.object(blocks, 'qualification', return_value=(context, self.spec, records)), \
             patch.object(blocks.runner, 'prerequisite_outcomes', return_value=outcomes):
            coverage = blocks.comparison_coverage(self.bound, self.bound)
            self.assertEqual(coverage['candidates'], context['candidates'])
            self.assertTrue(all(edge['status'] == 'invalid' for edge in coverage['comparisons']
                                if edge['reference'] == explicit))
            outcomes[explicit]['status'] = 'blocked'
            with self.assertRaises(ValueError):
                blocks.comparison_coverage(self.bound, self.bound)
            outcomes[explicit]['status'] = 'passed'
            outcomes['incumbent']['status'] = 'invalid'
            with self.assertRaises(ValueError):
                blocks.comparison_coverage(self.bound, self.bound)

    def test_confirmation_keeps_three_distinct_protections_and_requires_each(self):
        import time_blocks
        public = {'sha256': h('public'), 'policy': None, 'role': 'candidate'}
        manifest = deployment.holdout_manifest(self.spec, 'incremental-many', public)
        self.assertEqual(manifest['protections'], ['baseline', 'incumbent', 'selected-hidden'])
        self.assertEqual([manifest['arms'][a]['sha256'] for a in manifest['protections']],
                         [h('baseline'), h('accepted'), h('candidate')])
        self.assertEqual(manifest['arms']['selected-hidden']['policy'], routing.policy('incremental-many'))
        cell = manifest['panels']['C'][0]
        context = {'kind': deployment.KIND, 'stage': 'C'}
        pairs = [{'id': 'pair-' + arm, 'reference': arm, 'candidate': deployment.PUBLIC,
                  'reference_role': 'protected'} for arm in manifest['protections']]
        expected = {'cells': [{'id': pair['id'], 'workload': cell['id'], 'comparisons': [pair]} for pair in pairs]}
        rows = [{'cell_id': pair['id'], 'comparison_id': pair['id'], 'candidate_arm': deployment.PUBLIC,
                 'reference_arm': pair['reference'], 'reference_role': 'protected', 'attempted_slots': 24,
                 'conditional_nonregression_pass': True, 'excess_median_interval': {'upper': 0},
                 'normalized_regret_median_interval': {'upper': 1}} for pair in pairs]
        def decide(rows):
            analysis = {'missing_monitor_evidence': False, 'scientific_or_execution_failure': False, 'summary': rows}
            with patch.object(blocks, 'load_context', return_value=(context, manifest)), \
                 patch.object(deployment, 'timing_spec', return_value=expected), \
                 patch.object(blocks, 'comparison_coverage', return_value={'comparisons': []}), \
                 patch.object(time_blocks, 'load_manifest', return_value=({'spec': expected}, {})), \
                 patch.object(time_blocks, 'analyze', return_value=analysis):
                return routing.unseal(deployment.decide({'context': self.bound, 'qualification': self.bound},
                                                        self.bound, self.bound, self.bound))
        self.assertTrue(decide(rows)['conditional_nonregression_pass'])
        for index in range(3):
            self.assertFalse(decide(rows[:index] + rows[index+1:])['conditional_nonregression_pass'])
            bad = copy.deepcopy(rows)
            bad[index]['excess_median_interval']['upper'] = .000001
            self.assertFalse(decide(bad)['conditional_nonregression_pass'])
        with self.assertRaises(ValueError):
            decide(rows + [rows[0]])

    def test_terminal_retention_binds_actual_decision_and_both_platform_builds(self):
        decision = {'finalist': 'incumbent', 'timing_manifest': self.bound,
                    'records': self.bound, 'monitor_receipt': self.bound}
        source = {'revision': 'a' * 40, 'artifacts': {'Darwin': h('accepted'), 'Linux': h('accepted-linux')},
                  'production_tokens_sha256': h('accepted-source'), 'artifact_inputs_sha256': h('inputs')}
        evidence = self.write({'actual': 'operational review'})
        builds = {host: self.write({'status': 'passed', 'platform': host, 'revision': source['revision'],
            'plugin_sha256': artifact, 'production_tokens_sha256': source['production_tokens_sha256'],
            'artifact_inputs_sha256': source['artifact_inputs_sha256'], 'evidence': evidence})
            for host, artifact in source['artifacts'].items()}
        review = {'kind': deployment.TERMINAL + '_operational_review', 'status': 'passed',
                  'outcome': 'retained', 'source_decision_sha256': routing.identity(decision),
                  'incumbent_source': self.write(source), 'platform_builds': builds, 'evidence': [evidence]}
        review['independent_export_review'] = self.write({'kind': deployment.TERMINAL + '_export_review',
            'status': 'passed', 'source_decision_sha256': routing.identity(decision),
            'incumbent_source_sha256': review['incumbent_source']['sha256'],
            'stage_family': 'many', 'outcome': 'retained', 'evidence': [evidence],
            'prior_manifest_sha256': routing.identity(self.spec), 'incumbent_revision': source['revision'],
            'incumbent_artifacts': source['artifacts'], 'incumbent_source_identity': {key: source[key] for key in
                ['production_tokens_sha256', 'artifact_inputs_sha256']}})
        inputs = {'config': self.write({'context': self.bound}), 'decision': self.write(routing.seal(decision)),
                  'operational_review': self.write(review)}
        with patch.object(blocks, 'decide', return_value=routing.seal(decision)):
            value = deployment.terminal_stage(inputs)
            self.assertEqual(value['incumbent_artifacts'], source['artifacts'])
            self.assertEqual(deployment.validate_terminal(value), value)
            with patch.object(deployment, 'terminal_stage', side_effect=AssertionError('remote raw access')):
                self.assertEqual(deployment.validate_terminal(value, portable=True), value)
                forged = copy.deepcopy(value)
                forged['export_review']['report']['source_decision_sha256'] = h('forged')
                with self.assertRaises(ValueError): deployment.validate_terminal(forged, portable=True)
            with self.assertRaises(ValueError):
                deployment.validate_terminal({**value, 'incumbent_revision': 'b' * 40})
            for mutation in ({'outcome': 'accepted'}, {'source_decision_sha256': h('changed')},
                             {'platform_builds': {'Darwin': builds['Darwin']}}):
                with self.assertRaises(ValueError):
                    deployment.terminal_stage({**inputs, 'operational_review': self.write({**review, **mutation})})

    def test_changed_remote_recomputed_result_is_rejected(self):
        review = copy.deepcopy(self.review)
        review['remote_validation'] = self.write({'status': 'passed',
            'plan_sha256': routing.identity(self.plan), 'result_sha256': routing.identity(self.summary),
            'recomputed_result': {**self.summary, 'shortlist': ['incumbent']}})
        with self.assertRaisesRegex(ValueError, 'exact independently recomputed'):
            blocks.declare({**self.inputs, 'discovery_review': self.write(review)}, self.design)


if __name__ == '__main__':
    unittest.main()
