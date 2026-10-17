"""Synthetic activation/C contexts; no production artifacts or holdout bodies."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import routing
import routing_blocks as blocks
import routing_block_deployment as deployment
import routing_run as runner
import time_blocks
from native_io import case_header
from test_native import case_bytes
from test_routing import h
import test_routing_blocks
import test_routing_deployment
from test_routing_run import TRACE
import test_routing_run

REAL_EVIDENCE = runner.evidence


class PublicBlocks(unittest.TestCase):
    def setUp(self):
        self.b = test_routing_blocks.BlockContext()
        self.b.setUp()
        self.addCleanup(self.b.doCleanups)
        self.a = test_routing_deployment.DeploymentGate()
        self.a.setUp()
        self.addCleanup(self.a.doCleanups)
        self.write, self.root = self.b.write, self.b.root
        self.b.spec['candidate_source_identity'].update(
            production_tokens_sha256=routing.identity(self.a.old_tokens), artifact_inputs_sha256=h('inputs'))
        self.b.plan['tournament'] = routing.seal(self.b.spec)
        self.b.summary['plan_sha256'] = routing.identity(self.b.plan)
        self.b.review.update(plan_sha256=routing.identity(self.b.plan), result_sha256=routing.identity(self.b.summary),
            production_tokens_sha256=routing.identity(self.a.old_tokens), remote_validation=self.write({
                'status': 'passed', 'plan_sha256': routing.identity(self.b.plan), 'result_sha256': routing.identity(self.b.summary)}))
        for key, value in [('manifest', self.b.spec), ('discovery_plan', self.b.plan), ('discovery_result', self.b.summary)]:
            self.b.inputs[key] = self.write(routing.seal(value))
        self.b.inputs['discovery_review'] = self.write(self.b.review)
        self.b.context = blocks.declare(self.b.inputs, self.b.design)
        self.b.context_ref = self.write(routing.seal(self.b.context))
        self.b_config, self.b_spec, self.b_analysis, self.b_bound = self.b.decision_fixture()
        for row in self.b_analysis['summary']:
            if row['candidate_arm'] == 'h8-k0' and row['reference_role'] == 'internal':
                row['normalized_regret_median_interval']['upper'] = .1
        self.manifests = {self.b_bound['path']: {'spec': self.b_spec, 'bindings': []}}
        self.analyses = {self.b_bound['path']: self.b_analysis}
        verified_proof = self.write(routing.seal({'synthetic': 'verified proof'}))
        def verified_evidence(path, *_args, **_kwargs):
            value = json.loads(path.read_text())
            value.setdefault('scoped_replay', verified_proof)
            return value, {}
        self.enterContext(patch.object(runner, 'evidence', side_effect=verified_evidence))
        self.enterContext(patch.object(time_blocks, 'load_manifest', side_effect=lambda path: (self.manifests[str(path)], {})))
        self.enterContext(patch.object(time_blocks, 'analyze', side_effect=lambda path, *_: copy.deepcopy(self.analyses[str(path)])))
        decision = blocks.decide(self.b_config, self.b_bound, self.b_bound, self.b_bound)
        self.assertEqual(decision['value']['finalist'], 'h8-k0')
        declared = deployment.declare(self.b.context_ref, {**self.b.design, 'attempts': 24, 'session_id': 'C-fixed'})
        self.declaration = self.write(routing.seal(declared))
        self.trace = {'preset': 'public_auto', 'effective_policy': routing.policy('h8-k0')}
        for name, report in self.a.reports.items():
            report.update(kind=deployment.KIND + ':' + name, hidden_sha256=h('candidate'),
                          declaration_sha256=routing.identity(declared))
        self.a.reports['source_review']['public_trace'] = self.trace
        equivalent = self.a.reports['equivalence']
        equivalent['public_trace'] = self.trace
        calls = copy.deepcopy(equivalent['cells'][0]['calls'])
        for call in calls:
            if call['arm'] == 'hidden': call['plugin_sha256'] = h('candidate')
        equivalent['cells'] = [{**cell, 'calls': copy.deepcopy(calls)} for cell in declared['non_c_cells']]
        self.a.packet.update(kind=deployment.KIND, hidden_sha256=h('candidate'))
        self.a.bind_reports()
        self.inputs = {'declaration': self.declaration, 'b_config': self.write(self.b_config),
                       'b_decision': self.write(decision), 'activation': self.write(self.a.packet)}
        partial = copy.deepcopy(self.a.packet)
        del partial['reports']['equivalence']
        self.partial_inputs = {**self.inputs, 'activation': self.write(partial)}
        equivalent['collections'], equivalent['cells'] = [], []
        for stage in ['A', 'B']:
            cohort = deployment.equivalence_context(self.partial_inputs, stage)
            cohort_ref = self.write(routing.seal(cohort))
            records = self.qualified(cohort, cohort_ref)
            for row in [*records['prerequisites'], *records['records']]:
                row.update(science_sha256=h(row['cell']), solver_signature_sha256=h('solver-signature'),
                           resource_basis='actual traced prerequisite')
            for cell in cohort['cells']:
                cid = cell['id']
                proof = self.write(routing.seal({'prerequisites': [row for row in records['prerequisites'] if row['cell'] == cid],
                    'repeat_checks': [row for row in records['records'] if row['cell'] == cid]}))
                wrapper = self.write({'scoped_replay': {'path': Path(proof['path']).name, 'sha256': proof['sha256']}})
                records['qualified_wrappers'][cid] = {'path': Path(wrapper['path']).name, 'sha256': wrapper['sha256']}
            bound = self.write(records)
            _, cells, _ = deployment.equivalence_calls(cohort_ref, bound)
            equivalent['collections'].append({'context': cohort_ref, 'records': bound})
            equivalent['cells'].extend(cells)
        self.a.packet['reports']['equivalence'] = self.write(equivalent)
        self.inputs['activation'] = self.write(self.a.packet)
        self.context = deployment.derive(self.inputs)
        self.context_ref = self.write(routing.seal(self.context))
        self.view = self.context['manifest']

    def binary(self, name, value):
        path = self.root / name
        path.write_bytes(value if isinstance(value, bytes) else value.encode())
        return blocks.reference(path)

    def qualified(self, context, bound):
        old = self.b.context, self.b.context_ref, self.b.spec
        self.b.context, self.b.context_ref, self.b.spec = context, bound, context['manifest']
        try:
            records = self.b.qualified()
        finally:
            self.b.context, self.b.context_ref, self.b.spec = old
        records['stage'] = context['stage']
        return records

    def timing_config(self):
        records = self.qualified(self.context, self.context_ref)
        cells = {}
        for cell in self.context['cells']:
            cid = cell['id']
            arms = {}
            for arm in blocks.required_arms(self.context, cell):
                plugin = (self.a.packet['public_plugin'] if arm == deployment.PUBLIC else
                          self.binary('plugin-' + arm, 'baseline' if arm == 'baseline' else 'candidate'))
                arms[arm] = {'plugin': plugin, 'input': self.binary(cid + arm + '.case', 'prepared-' + cid + arm)}
            cells[cid] = {'source': self.binary(cid + '.case', 'C'), 'arms': arms}
        return {'context': self.context_ref, 'qualification': self.write(records),
                'harness': self.binary('harness', 'harness'), 'monitor': None, 'cells': cells}

    def test_portfolio_declaration_keeps_separate_24_block_c_and_binds_parallel_source(self):
        for schema in (2, 3):
            with self.subTest(schema=schema):
                self.check_portfolio_declaration(schema)

    def check_portfolio_declaration(self, schema):
        spec, _, _, _, inputs = self.b.portfolio_inputs(schema)
        context = blocks.declare(inputs, self.b.design)
        bound = self.write(routing.seal(context))
        before = copy.deepcopy(spec['panels']['C'])
        with patch.object(runner, 'inspect_case') as inspect:
            declaration = deployment.declare(bound,
                {**self.b.design, 'attempts': 24, 'session_id': 'portfolio-C'})
            inspect.assert_not_called()
        self.assertEqual(declaration['design']['attempts'], 24)
        self.assertEqual(declaration['contract'], deployment.CONTRACT)
        self.assertIn('routing_parallel.py', {Path(ref['path']).name for ref in declaration['sources']})
        self.assertEqual(spec['panels']['C'], before)
        self.assertEqual({c['stage'] for c in declaration['non_c_cells']}, {'A', 'B'})
        with self.assertRaises(ValueError):
            deployment.declare(bound, {**self.b.design, 'attempts': 12})
        # A valid new B context does not authorize C or reuse an old activation.
        with self.assertRaises(ValueError):
            blocks.prerequisite_context(spec, 'C', None, bound)
        crossed = {**self.partial_inputs, 'declaration': self.write(routing.seal(declaration))}
        with patch.object(runner, 'inspect_case') as inspect, self.assertRaises(ValueError):
            deployment.derive(crossed)
        inspect.assert_not_called()
        with self.assertRaises(ValueError):
            deployment.equivalence_context(crossed, 'C')

    def test_genuine_C_view_complete_activation_and_source_closure(self):
        self.assertEqual(blocks.load_context(self.context_ref), (self.context, self.view))
        self.assertEqual(self.context['cells'], self.b.spec['panels']['C'])
        self.assertEqual(self.view['protections'], ['baseline', 'incumbent'])
        self.assertEqual(self.view['arms']['incumbent']['policy'], routing.policy('h8-k0'))
        with self.assertRaises(ValueError): routing.validate_manifest(self.view)
        names = {Path(ref['path']).name for ref in blocks.context_bindings(self.context_ref, self.context)}
        self.assertTrue({'routing_block_deployment.py', 'routing_deployment.py', 'routing_blocks.py',
                         'routing_run.py', 'time_blocks.py'} <= names)
        with patch.object(blocks.platform, 'system', return_value='Darwin'):
            resolved = blocks.prerequisite_context(self.view, 'C', None, self.context_ref)
        self.assertEqual(resolved['candidates'], ['public-auto'])

    def test_real_B_reanalysis_and_fresh_activation_are_required(self):
        original_analysis = copy.deepcopy(self.b_analysis)
        for change in ['decision', 'observations', 'failed-activation', 'stale-declaration', 'trace']:
            inputs = copy.deepcopy(self.inputs)
            packet = copy.deepcopy(self.a.packet)
            if change == 'decision':
                decision = blocks.read(inputs['b_decision'], sealed=True)
                decision['finalist'] = 'h16-k0'
                inputs['b_decision'] = self.write(routing.seal(decision))
            elif change == 'observations':
                self.analyses[self.b_bound['path']]['summary'][0]['conditional_nonregression_pass'] = False
            elif change == 'stale-declaration':
                declared = blocks.read(inputs['declaration'], sealed=True)
                declared['sources'][0]['sha256'] = h('changed')
                inputs['declaration'] = self.write(routing.seal(declared))
            else:
                report = copy.deepcopy(self.a.reports['source_review'])
                if change == 'failed-activation': report['activation_only'] = False
                else: report['public_trace']['effective_policy'] = routing.policy('h16-k0')
                packet['reports']['source_review'] = self.write(report)
                inputs['activation'] = self.write(packet)
            with self.subTest(change=change), self.assertRaises(ValueError): deployment.derive(inputs)
            self.analyses[self.b_bound['path']] = copy.deepcopy(original_analysis)

    def test_C_dispatch_cannot_reuse_legacy_B_or_skip_activation_before_case_access(self):
        with patch.object(runner, 'inspect_case') as inspect:
            for context in [self.b.context_ref, self.write(routing.seal({**self.context, 'selected': 'h16-k0'}))]:
                with self.assertRaises(ValueError):
                    runner.run(self.view, 'C', None, Path('unused'), {}, {}, {}, self.root / 'never',
                               prerequisites_only=True, qualification_context=context)
            inspect.assert_not_called()

    def test_equivalence_context_is_non_c_two_artifacts_and_raw_records_are_checked(self):
        with self.assertRaises(ValueError): deployment.equivalence_context(self.partial_inputs, 'C')
        context = deployment.equivalence_context(self.partial_inputs, 'A')
        self.assertEqual(context['manifest']['execution_roles'], {'baseline': 'hidden', 'public-auto': 'public'})
        self.assertEqual(context['manifest']['arms']['baseline']['sha256'], h('candidate'))
        self.assertEqual(runner.input_policy(context['manifest'], 'baseline'), 'h8-k0')
        self.assertEqual(len(context['manifest']['arms']), 2)
        collection = self.a.reports['equivalence']['collections'][0]
        records = blocks.read(collection['records'])
        records['records'][0]['science_sha256'] = h('changed science')
        with self.assertRaises(ValueError): deployment.equivalence_calls(collection['context'], self.write(records))
        with self.assertRaises(ValueError): blocks.timing_spec({'context': collection['context'],
            'qualification': collection['records']})

    def test_public_request_and_resource_policy_are_independent(self):
        source = self.root / 'synthetic.case'
        source.write_bytes(case_bytes())
        prepared = self.root / 'public.case'
        routing.prepare(source, prepared, runner.input_policy(self.view, deployment.PUBLIC), 1)
        runner.verify_public_request(prepared, self.view, deployment.PUBLIC)
        with prepared.open('rb') as stream: _, arguments = case_header(stream)
        self.assertIn(b'solver=auto', arguments)
        self.assertFalse(any(arg.startswith(b'inject=') for arg in arguments))
        self.assertEqual(runner.reader_arm(self.view, deployment.PUBLIC), 'auto')
        effective, preset = runner.resource_policy(self.view, deployment.PUBLIC)
        self.assertEqual((effective, preset), ('h8-k0', 'public_auto'))
        trace = TRACE.replace('preset=routing_h4_k0', 'preset=public_auto')
        result = runner.resource_diagnostics(trace, effective, False, {}, expected_preset=preset)
        self.assertEqual(result['resource_checks']['setup_caps'], 'passed')
        for bad in ['', TRACE, trace.replace('setup_work=12', 'setup_work=1000'),
                    trace + 'FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=16|samples=0\n']:
            with self.assertRaises(ValueError): runner.resource_diagnostics(bad, effective, False, {}, expected_preset=preset)
        for args in [['solver=auto', 'inject=routing_h8_k0'], ['solver=lsmr'], ['solver=auto', 'solver=auto']]:
            bad = self.root / 'bad.case'
            bad.write_bytes(case_bytes([b'estimate-ols', *[arg.encode() for arg in args]]))
            with self.assertRaises(ValueError): runner.verify_public_request(bad, self.view, deployment.PUBLIC)

    def test_real_producer_hook_collects_two_calls_each_and_actual_signatures(self):
        fixture = test_routing_run.RoutingProducer()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        cohort = deployment.equivalence_context(self.partial_inputs, 'A')
        view = copy.deepcopy(cohort['manifest'])
        cell = {**fixture.cell, 'comparators': []}
        view['panels']['A'] = [cell]
        view['arms'][deployment.PUBLIC]['sha256'] = blocks.digest(fixture.baseline)
        fixture.evidence.update(arms=view['arms'], manifest_sha256=routing.identity(view))
        fixture.witness.write_text(json.dumps(fixture.evidence))
        calls = []
        def native(command, log, timeout):
            with Path(command[-2]).open('rb') as stream: _, args = case_header(stream)
            fields = dict(arg.decode().split('=', 1) for arg in args[1:])
            public = Path(command[-4]) == fixture.baseline
            self.assertEqual(fields['solver'], 'auto')
            self.assertEqual(fields.get('inject'), None if public else 'routing_h8_k0')
            traced = 'FEREG_SOLVER_TRACE=1' in command
            calls.append((public, traced))
            raw = test_routing_run.result('auto')
            for row in range(2):
                for column in [8, 9]:
                    raw['matrices']['RSOLVER']['bits'][16 * row + column] = test_routing_run.bits(float(len(calls)))
            target = Path(command[-1])
            target.write_text(json.dumps({**raw, 'columns': []}))
            fixture.data.tofile(target.with_suffix('.json.columns'))
            preset = 'public_auto' if public else 'routing_h8_k0'
            log.write_text('NATIVE_SECONDS=0.1\n' + (TRACE.replace('routing_h4_k0', preset) if traced else ''))
            return {'wall_s': .1, 'maxrss_bytes': 10}
        resolved = {'candidates': [deployment.PUBLIC], 'selection_sha256': routing.identity(cohort), 'bindings': []}
        with patch.object(blocks, 'prerequisite_context', return_value=resolved), \
             patch.object(runner, 'evidence', side_effect=REAL_EVIDENCE), \
             patch.object(runner, 'inspect_case', return_value=fixture.meta), \
             patch.object(runner.cmgschur, 'oracle_check'), \
             patch.object(runner.cmgschur, 'run_process', side_effect=native):
            records = runner.run(view, 'A', None, fixture.harness,
                {'baseline': fixture.plugin, deployment.PUBLIC: fixture.baseline}, {cell['id']: fixture.source},
                {cell['id']: fixture.witness}, fixture.root / 'public-science', prerequisites_only=True,
                qualification_context=self.context_ref)
        self.assertEqual(records['status'], 'complete')
        self.assertEqual(sorted(calls), [(False, False), (False, True), (True, False), (True, True)])
        rows = [*records['prerequisites'], *records['records']]
        self.assertEqual({row['solver_signature_sha256'] for row in rows},
                         {routing.identity(runner.exact_view(test_routing_run.result('auto'), False)['matrices']['RSOLVER'])})
        self.assertTrue(all(row['resource_checks']['setup_caps'] == 'passed' for row in rows))

    def test_only_fresh_optional_unavailability_can_remove_a_C_pair(self):
        config = self.timing_config()
        for arm in ['lsmr', 'baseline', 'incumbent']:
            records = blocks.read(config['qualification'])
            cid = self.context['cells'][0]['id']
            before = next(row for row in records['prerequisites'] if row['cell'] == cid and row['arm'] == arm)
            before.update(status='unavailable', failure={'kind': 'timeout', 'dispatched': True,
                'call_limit_s': 120., 'elapsed_s': 120.1, 'log_sha256': h('timeout'),
                'prepared_sha256': before['prepared_sha256']})
            after = next(row for row in records['records'] if row['cell'] == cid and row['arm'] == arm)
            after.update(status='not_run', blocked_by_sha256=routing.observation_identity(before))
            changed = copy.deepcopy(config)
            changed['qualification'] = self.write(records)
            if arm != 'lsmr':
                with self.assertRaises(ValueError): deployment.timing_spec(changed)
            else:
                del changed['cells'][cid]['arms'][arm]
                self.assertEqual(len(deployment.timing_spec(changed)['cells']), 2)
                coverage = blocks.comparison_coverage(self.context_ref, changed['qualification'])
                self.assertEqual([edge['reference'] for edge in coverage['comparisons'] if edge['status'] == 'unavailable'], ['lsmr'])

    def test_C_spec_uses_all_separate_pairs_and_balanced_24_blocks(self):
        config = self.timing_config()
        spec = deployment.timing_spec(config)
        self.assertEqual(len(spec['cells']), 3 * len(self.context['cells']))
        self.assertTrue(all(len(cell['arms']) == 2 and len(cell['comparisons']) == 1 for cell in spec['cells']))
        self.assertEqual({cell['comparisons'][0]['reference_role'] for cell in spec['cells']}, {'protected', 'internal'})
        schedule = time_blocks.schedule(spec)
        for cell in spec['cells']:
            rows = [row for row in schedule['blocks'] if row['cell_id'] == cell['id']]
            self.assertEqual(len(rows), 24)
            counts = {}
            for row in rows:
                key = tuple(row['arms'])
                counts[key] = counts.get(key, 0) + 1
            self.assertEqual(sorted(counts.values()), [12, 12])
        with self.assertRaises(ValueError): blocks.timing_spec(config)
        with self.assertRaises(ValueError): deployment.timing_spec({**config, 'cells': {}})

    def test_final_gate_has_no_fallback_and_requires_every_attainable_edge(self):
        config = self.timing_config()
        spec = deployment.timing_spec(config)
        bound = self.write({'synthetic': 'C records'})
        self.manifests[bound['path']] = {'spec': spec, 'bindings': []}
        analysis = {'missing_monitor_evidence': False, 'scientific_or_execution_failure': False,
            'summary': [{'cell_id': cell['id'], 'comparison_id': cell['comparisons'][0]['id'],
                'candidate_arm': deployment.PUBLIC, 'reference_arm': cell['comparisons'][0]['reference'],
                'reference_role': cell['comparisons'][0]['reference_role'], 'attempted_slots': 24,
                'conditional_nonregression_pass': True, 'excess_median_interval': {'upper': -.001},
                'normalized_regret_median_interval': {'upper': .9}} for cell in spec['cells']]}
        self.analyses[bound['path']] = analysis
        result = routing.unseal(deployment.decide(config, bound, bound, bound))
        self.assertEqual(result['status'], 'conditional_pass')
        self.assertFalse(result['promotion'])
        for failure in ['missing', 'inconclusive', 'regression', 'attempts', 'monitor', 'science']:
            changed = copy.deepcopy(analysis)
            if failure == 'missing': changed['summary'].pop()
            elif failure == 'inconclusive': changed['summary'][0]['excess_median_interval']['upper'] = None
            elif failure == 'regression': changed['summary'][0]['normalized_regret_median_interval']['upper'] = 1.01
            elif failure == 'attempts': changed['summary'][0]['attempted_slots'] = 22
            elif failure == 'monitor': changed['missing_monitor_evidence'] = True
            else: changed['scientific_or_execution_failure'] = True
            self.analyses[bound['path']] = changed
            with self.subTest(failure=failure):
                result = routing.unseal(deployment.decide(config, bound, bound, bound))
                self.assertEqual(result['status'], 'not_confirmed')
                self.assertNotIn('finalist', result)
        self.analyses[bound['path']] = analysis
        self.manifests[bound['path']]['spec'] = {**spec, 'cells': spec['cells'][:-1]}
        with self.assertRaisesRegex(ValueError, 'complete paired C'): deployment.decide(config, bound, bound, bound)


if __name__ == '__main__':
    unittest.main()
