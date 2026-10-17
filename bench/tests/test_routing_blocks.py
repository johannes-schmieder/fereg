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
from test_routing import h, manifest, portfolio_manifest


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

    def portfolio_inputs(self, schema=2):
        import routing_parallel as parallel
        from test_routing_discovery import partition42
        spec = partition42() if schema == 3 else portfolio_manifest()
        candidate = routing.portfolio_candidate(spec)
        for stage in (['B'] if schema == 3 else ['A', 'B']):
            cell = spec['panels'][stage][0]
            spec['panels'][stage] = [{**cell, 'id': f'{stage}-portfolio-{i}',
                'case_sha256': h(f'{stage}-portfolio-case-{i}'),
                'body_sha256': h(f'{stage}-portfolio-body-{i}')} for i in range(42)]
        spec['candidate_source_identity'] = {'production_tokens_sha256': h('production'),
            'plugin_sha256': h('candidate'), 'artifact_inputs_sha256': h('inputs')}
        parent = {'kind': discovery.PORTFOLIO_KIND + '_plan', 'schema': 1,
            'design': discovery.DESIGN, 'tournament': routing.seal(spec),
            'library_limits': discovery.LIBRARY_LIMITS, 'producer_tools': {'producer.py': h('producer')},
            'frozen_files': {}, 'output_root': str(self.root / 'discovery')}
        parent_ref, collector = self.write(routing.seal(parent)), self.write({'collector': 'frozen'})
        plan = {'kind': parallel.KIND + '_plan', 'schema': schema,
            'design': parallel.PARTITION_DESIGN if schema == 3 else parallel.PORTFOLIO_DESIGN,
            'parent': routing.seal(parent), 'parent_sha256': routing.identity(parent), 'parent_plan': parent_ref,
            'output_root': str(self.root / 'portfolio-rounds'), 'producer_tools': parent['producer_tools'],
            'receipt_generator': collector,
            'frozen_files': {ref['path']: ref['sha256'] for ref in [parent_ref, collector]}}
        if schema == 3:
            plan['partitions'] = discovery.partitions(spec)
        reports = []
        labels = [(n, shard) for n in (1, 2, 3) for shard in ((1, 2) if schema == 3 else (None,))]
        for n, shard in labels:
            root = Path(plan['output_root']) / f'round-{n}'
            root = root / f'shard-{shard}' / 'attempt-1' if schema == 3 else root / 'attempt-1'
            refs = {key: {'path': str(root / relative), 'sha256': h(f'{n}-{shard}-{key}')}
                    for key, relative in [('collection', 'collection/collection.json'),
                        ('isolation', 'isolation/receipt.json'), ('prerequisites', 'prerequisites/prerequisites.json')]}
            reports.append({'kind': parallel.KIND + '_attempt', 'parallel_plan_sha256': routing.identity(plan),
                'logical_round': n, 'attempt': 1, 'status': 'complete', **refs,
                'science_discrete_sha256': h(f'stable-discrete-{shard}'), 'accounted_wallclock_s': 100.})
            if schema == 3:
                reports[-1].update(shard=shard, partition_sha256=routing.identity(plan['partitions'][shard-1]),
                    model_discrete_sha256=h(f'stable-model-{shard}'), worker_route_differences=[])
        collection = {'kind': parallel.KIND + '_collection', 'schema': schema,
                      'plan_sha256': routing.identity(plan), 'attempts': reports}
        receipts = [report['isolation'] for report in reports]
        ranking = [{'arm': a, 'baseline_violations': 0, 'worst_internal_normalized': .5}
                   for a in [candidate, 'baseline']]
        summary = {'kind': discovery.PORTFOLIO_KIND + '_shortlist', 'schema': schema,
            'experiment_kind': 'portfolio', 'protocol_source': parallel.KIND,
            'status': 'discovery_only', 'promotion': False, 'plan_sha256': routing.identity(plan),
            'collection': collection, 'isolation': receipts, 'collection_sha256': routing.identity(collection),
            'isolation_sha256': routing.identity(receipts), 'ranking': ranking,
            **parallel.portfolio_screen(ranking, candidate=candidate)}
        raw = self.write({'raw': 'complete local science, scheduler and timing observations'})
        shared = {'status': 'passed', 'plan_sha256': routing.identity(plan),
                  'result_sha256': routing.identity(summary), 'collection_sha256': summary['collection_sha256'],
                  'isolation_sha256': summary['isolation_sha256']}
        review = {'kind': blocks.KIND + '_discovery_review', **shared,
            'production_tokens_sha256': h('production'), 'raw_evidence': [raw],
            'remote_validation': self.write({**shared, 'recomputed_result': summary}),
            'independent_raw_review': self.write({**shared, 'evidence': [raw],
                'prerequisites': [report['prerequisites'] for report in reports]})}
        inputs = {**self.inputs, 'manifest': self.write(routing.seal(spec)),
            'discovery_plan': self.write(routing.seal(plan)), 'discovery_result': self.write(routing.seal(summary)),
            'discovery_review': self.write(review)}
        return spec, plan, summary, review, inputs

    def test_partitioned_handoff_rederives_context_and_binds_independent_review(self):
        import routing_block_deployment as deployment
        import routing_deployment as activation
        spec, plan, summary, review, inputs = self.portfolio_inputs(3)
        with patch.object(runner, 'inspect_case') as inspect:
            context = blocks.declare(inputs, self.design)
            bound = self.write(routing.seal(context))
            self.assertEqual(blocks.load_context(bound), (context, spec))
            self.assertEqual(context['candidates'], summary['shortlist'])
            self.assertIn(review['independent_raw_review'], blocks.context_bindings(bound, context))
            declared = deployment.declare(bound, {**self.design, 'attempts': 24})
            self.assertEqual(declared['original_manifest_sha256'], routing.identity(spec))
            self.assertEqual(activation.origin(routing.seal(spec), block_context=bound), spec)
            with self.assertRaises(ValueError): activation.origin(routing.seal(spec))
            with self.assertRaises(ValueError): activation.declare(routing.seal(spec))
            for stage in ('A', 'C'):
                with self.assertRaises(ValueError): blocks.prerequisite_context(spec, stage, None, bound)
            with self.assertRaises(ValueError):
                activation.origin(routing.seal(spec), block_context=self.context_ref)
            forged = self.write(routing.seal({**context, 'candidates': ['baseline']}))
            with self.assertRaises(ValueError): deployment.declare(forged, {**self.design, 'attempts': 24})
            inspect.assert_not_called()
        self.assertEqual([part['inclusive_allocation_budget_s'] for part in plan['partitions']], [7280, 7120])

    def test_partitioned_handoff_rejects_incomplete_reordered_or_forged_allocations(self):
        _, plan, summary, _, _ = self.portfolio_inputs(3)
        for change in ('missing', 'reorder', 'shard', 'bool', 'descriptor', 'budget', 'nan',
                       'route-signature', 'model-signature', 'borrowed-proof', 'extra', 'receipt'):
            bad = copy.deepcopy(summary)
            reports = bad['collection']['attempts']
            if change == 'missing': reports.pop()
            elif change == 'reorder': reports[0], reports[1] = reports[1], reports[0]
            elif change == 'shard': reports[0]['shard'] = 2
            elif change == 'bool': reports[0]['shard'] = True
            elif change == 'descriptor': reports[0]['partition_sha256'] = h('other')
            elif change == 'budget': reports[1]['accounted_wallclock_s'] = 7121
            elif change == 'nan': reports[0]['accounted_wallclock_s'] = float('nan')
            elif change == 'route-signature': reports[2]['science_discrete_sha256'] = h('other')
            elif change == 'model-signature': reports[2]['model_discrete_sha256'] = h('other')
            elif change == 'borrowed-proof': reports[1]['prerequisites'] = reports[0]['prerequisites']
            elif change == 'extra': reports[0]['retry'] = 1
            else: bad['isolation'].reverse()
            with self.subTest(change=change), self.assertRaises(ValueError):
                bad['collection_sha256'] = routing.identity(bad['collection'])
                bad['isolation_sha256'] = routing.identity(bad['isolation'])
                blocks.portfolio_discovery_valid(plan, bad)
        for change in ('schema', 'partition-budget', 'policy'):
            bad = copy.deepcopy(plan)
            if change == 'schema': bad['schema'] = 2
            elif change == 'partition-budget': bad['partitions'][0]['inclusive_allocation_budget_s'] = 14400
            else:
                parent = routing.unseal(bad['parent'])
                spec = routing.unseal(parent['tournament'])
                spec['arms'][routing.PORTFOLIO_UNSEEDED]['policy'] = routing.policy(routing.PORTFOLIO_V1)
                parent['tournament'] = routing.seal(spec)
                bad.update(parent=routing.seal(parent), parent_sha256=routing.identity(parent))
            with self.subTest(change=change), self.assertRaises(ValueError):
                blocks.portfolio_discovery_valid(bad, summary)

    def test_partitioned_handoff_requires_reviewed_full_replay_and_rejects_baseline_only(self):
        import routing_parallel as parallel
        _, plan, summary, review, inputs = self.portfolio_inputs(3)
        for change in ('missing', 'stale', 'partial-proofs', 'remote-result', 'mutated-review'):
            bad = copy.deepcopy(review)
            if change == 'missing': bad.pop('independent_raw_review')
            elif change == 'remote-result':
                remote = blocks.read(bad['remote_validation'])
                remote['recomputed_result']['collection']['attempts'].pop()
                bad['remote_validation'] = self.write(remote)
            elif change == 'mutated-review':
                Path(bad['independent_raw_review']['path']).write_text('{}')
            else:
                independent = blocks.read(bad['independent_raw_review'])
                if change == 'stale': independent['result_sha256'] = h('other')
                else: independent['prerequisites'].pop()
                bad['independent_raw_review'] = self.write(independent)
            with self.subTest(change=change), self.assertRaises((ValueError, KeyError)):
                blocks.declare({**inputs, 'discovery_review': self.write(bad)}, self.design)
        summary['ranking'][0]['baseline_violations'] = 1
        summary.update(parallel.portfolio_screen(summary['ranking'], candidate=routing.PORTFOLIO_UNSEEDED))
        blocks.portfolio_discovery_valid(plan, summary)
        with self.assertRaisesRegex(ValueError, 'no Mac B'):
            blocks.declare({**inputs, 'discovery_result': self.write(routing.seal(summary))}, self.design)

    def test_portfolio_closed_handoff_requires_fresh_mac_science_and_no_body_access(self):
        for schema in (2, 3):
            with self.subTest(schema=schema):
                self.check_portfolio_fresh_mac_science(schema)

    def check_portfolio_fresh_mac_science(self, schema):
        spec, _, summary, _, inputs = self.portfolio_inputs(schema)
        with patch.object(runner, 'inspect_case') as inspect:
            context = blocks.declare(inputs, self.design)
            inspect.assert_not_called()
        self.assertEqual(context['candidates'], summary['shortlist'])
        bound = self.write(routing.seal(context))
        with patch.object(blocks.platform, 'system', return_value='Linux'), self.assertRaises(ValueError):
            blocks.prerequisite_context(spec, 'B', None, bound)
        with patch.object(blocks.platform, 'system', return_value='Darwin'):
            self.assertEqual(blocks.prerequisite_context(spec, 'B', None, bound)['candidates'], summary['shortlist'])
        with self.assertRaises(ValueError):
            blocks.prerequisite_context(spec, 'C', None, bound)
        names = {Path(ref['path']).name for ref in blocks.context_bindings(bound, context)}
        self.assertIn('routing_parallel.py', names)
        old = self.spec, self.context, self.context_ref
        self.spec, self.context, self.context_ref = spec, context, bound
        try:
            records = self.qualified()
            blocks.qualification(bound, self.write(records))
            records['platform'] = 'Linux-node'
            with self.assertRaises(ValueError): blocks.qualification(bound, self.write(records))
            import routing_science_completion as completion
            with self.assertRaisesRegex(ValueError, 'fresh Mac science'):
                blocks.qualification(bound, self.write(routing.seal({'kind': completion.KIND})))
        finally:
            self.spec, self.context, self.context_ref = old

    def test_portfolio_rejects_old_campaign_cross_kind_and_local_proof_forgery(self):
        spec, plan, summary, _, inputs = self.portfolio_inputs()
        for key in ['discovery_plan', 'discovery_result', 'discovery_review']:
            with self.subTest(old=key), self.assertRaises((ValueError, KeyError)):
                blocks.declare({**inputs, key: self.inputs[key]}, self.design)
        with self.assertRaises(ValueError):
            blocks.declare({**inputs, 'manifest': self.inputs['manifest']}, self.design)
        for kind in ['wrong-kind', 'old-schema', 'old-protocol', 'missing-round', 'borrowed-proof',
                     'repeat-round', 'extra-attempt', 'discrete', 'budget', 'wrong-receipt']:
            bad = copy.deepcopy(summary)
            reports = bad['collection']['attempts']
            if kind == 'wrong-kind': bad['kind'] = discovery.KIND + '_shortlist'
            elif kind == 'old-schema': bad['schema'] = 1
            elif kind == 'old-protocol': bad['protocol_source'] = discovery.KIND
            elif kind == 'missing-round': reports.pop()
            elif kind == 'borrowed-proof': reports[1]['prerequisites'] = reports[0]['prerequisites']
            elif kind == 'repeat-round': reports[1]['logical_round'] = 1
            elif kind == 'extra-attempt': reports[0]['attempt'] = 2
            elif kind == 'discrete': reports[0]['science_discrete_sha256'] = h('changed')
            elif kind == 'budget': reports[0]['accounted_wallclock_s'] = 14401
            else: bad['isolation'][0] = bad['isolation'][1]
            bad['collection_sha256'] = routing.identity(bad['collection'])
            bad['isolation_sha256'] = routing.identity(bad['isolation'])
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                blocks.portfolio_discovery_valid(plan, bad)
        for field in ['proof', 'legacy_window']:
            with self.subTest(field=field), self.assertRaises(ValueError):
                blocks.portfolio_discovery_valid({**plan, field: {}}, summary)

    def test_portfolio_requires_recomputed_remote_result_and_independent_raw_review(self):
        _, _, summary, review, inputs = self.portfolio_inputs()
        for kind in ['missing-review', 'missing-evidence', 'wrong-proof', 'status-only-remote', 'changed-result']:
            bad = copy.deepcopy(review)
            if kind == 'missing-review': bad.pop('independent_raw_review')
            elif kind in ['missing-evidence', 'wrong-proof']:
                independent = blocks.read(bad['independent_raw_review'])
                independent['evidence' if kind == 'missing-evidence' else 'prerequisites'] = []
                bad['independent_raw_review'] = self.write(independent)
            else:
                remote = blocks.read(bad['remote_validation'])
                if kind == 'status-only-remote': remote.pop('recomputed_result')
                else: remote['recomputed_result']['ranking'][0]['arm'] = 'h16-k1'
                bad['remote_validation'] = self.write(remote)
            with self.subTest(kind=kind), self.assertRaises((ValueError, KeyError)):
                blocks.declare({**inputs, 'discovery_review': self.write(bad)}, self.design)

    def test_portfolio_heuristic_rejection_cannot_open_mac_b(self):
        import routing_parallel as parallel
        _, plan, summary, _, inputs = self.portfolio_inputs()
        summary['ranking'][0]['baseline_violations'] = 1
        summary.update(parallel.portfolio_screen(summary['ranking']))
        blocks.portfolio_discovery_valid(plan, summary)
        with self.assertRaisesRegex(ValueError, 'no Mac B'):
            blocks.declare({**inputs, 'discovery_result': self.write(routing.seal(summary))}, self.design)

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

    def legacy_linux_plan(self):
        spec = copy.deepcopy(self.spec)
        spec['candidate_source_identity'] = {'production_tokens_sha256': h('production'),
            'archive_sha256': h('archive'), 'artifact_inputs_sha256': h('artifact inputs'),
            'commit': 'a' * 40}
        plugin = str(self.root / 'linux-candidate.plugin')
        return {**self.plan, 'tournament': routing.seal(spec),
                'plugins': {arm: plugin for arm in spec['candidates'] if arm != 'baseline'},
                'frozen_files': {plugin: h('candidate')}}

    def parallel_linux_plan(self, parent):
        import routing_parallel as parallel
        parent = {**parent, 'library_limits': discovery.LIBRARY_LIMITS,
                  'producer_tools': {'producer.py': h('producer')}}
        refs = {name: self.write({'bound': name})
                for name in ['parent_plan', 'proof', 'receipt_generator']}
        return {'kind': parallel.KIND + '_plan', 'schema': 1, 'design': parallel.DESIGN,
                'parent': routing.seal(parent), 'parent_sha256': routing.identity(parent),
                **refs, 'proof_sha256': h('proof'), 'output_root': str(self.root / 'parallel'),
                'producer_tools': parent['producer_tools'],
                'frozen_files': {**parent['frozen_files'],
                                 **{ref['path']: ref['sha256'] for ref in refs.values()}}}

    def rebound_discovery_inputs(self, plan, *, manifest=None):
        summary = {**self.summary, 'plan_sha256': routing.identity(plan)}
        validation = self.write({'status': 'passed', 'result_sha256': routing.identity(summary),
                                 'plan_sha256': routing.identity(plan)})
        review = {**self.review, 'plan_sha256': routing.identity(plan),
                  'result_sha256': routing.identity(summary), 'remote_validation': validation}
        return {**self.inputs, 'manifest': self.write(routing.seal(
                    self.spec if manifest is None else manifest)),
                'discovery_plan': self.write(routing.seal(plan)),
                'discovery_result': self.write(routing.seal(summary)),
                'discovery_review': self.write(review)}

    def test_legacy_linux_artifact_binding_accepts_original_and_parallel_metadata(self):
        for parallel in [False, True]:
            with self.subTest(parallel=parallel):
                plan = self.legacy_linux_plan()
                if parallel:
                    plan = self.parallel_linux_plan(plan)
                with patch.object(runner, 'inspect_case') as inspect:
                    context = blocks.declare(self.rebound_discovery_inputs(plan), self.design)
                    inspect.assert_not_called()
                self.assertEqual(context['candidates'], self.summary['shortlist'])

    def test_legacy_linux_binding_rejects_tampered_metadata_with_fresh_review_identities(self):
        changes = ['missing plugin', 'missing frozen', 'wrong frozen', 'mixed frozen',
                   'relative plugin', 'missing archive', 'invalid archive',
                   'missing artifact inputs', 'invalid artifact inputs',
                   'missing commit', 'short commit', 'nonhex commit',
                   'null explicit plugin', 'invalid explicit plugin', 'wrong explicit plugin']
        for parallel in [False, True]:
            for change in changes:
                with self.subTest(parallel=parallel, change=change):
                    plan = self.legacy_linux_plan()
                    spec = routing.unseal(plan['tournament'])
                    source = spec['candidate_source_identity']
                    arm = next(iter(plan['plugins']))
                    path = plan['plugins'][arm]
                    if change == 'missing plugin':
                        del plan['plugins'][arm]
                    elif change == 'missing frozen':
                        del plan['frozen_files'][path]
                    elif change == 'wrong frozen':
                        plan['frozen_files'][path] = h('other artifact')
                    elif change == 'mixed frozen':
                        other = str(self.root / 'different-linux.plugin')
                        plan['plugins'][arm] = other
                        plan['frozen_files'][other] = h('other artifact')
                    elif change == 'relative plugin':
                        plan['plugins'][arm] = 'relative.plugin'
                        plan['frozen_files']['relative.plugin'] = h('candidate')
                    elif change in ['missing archive', 'invalid archive']:
                        source.pop('archive_sha256')
                        if change == 'invalid archive':
                            source['archive_sha256'] = 'invalid'
                    elif change in ['missing artifact inputs', 'invalid artifact inputs']:
                        source.pop('artifact_inputs_sha256')
                        if change == 'invalid artifact inputs':
                            source['artifact_inputs_sha256'] = None
                    elif change == 'missing commit':
                        source.pop('commit')
                    elif change == 'short commit':
                        source['commit'] = 'a' * 39
                    elif change == 'nonhex commit':
                        source['commit'] = 'z' * 40
                    else:
                        source['plugin_sha256'] = {'null explicit plugin': None,
                            'invalid explicit plugin': 'invalid',
                            'wrong explicit plugin': h('other artifact')}[change]
                    plan['tournament'] = routing.seal(spec)
                    if parallel:
                        plan = self.parallel_linux_plan(plan)
                    with self.assertRaises(ValueError):
                        blocks.declare(self.rebound_discovery_inputs(plan), self.design)

    def test_missing_Mac_plugin_identity_cannot_use_linux_legacy_fallback(self):
        mac = copy.deepcopy(self.spec)
        del mac['candidate_source_identity']['plugin_sha256']
        for parallel in [False, True]:
            with self.subTest(parallel=parallel):
                plan = self.legacy_linux_plan()
                if parallel:
                    plan = self.parallel_linux_plan(plan)
                with self.assertRaises(ValueError):
                    blocks.declare(self.rebound_discovery_inputs(plan, manifest=mac), self.design)

    def test_parallel_outer_bindings_cannot_replace_original_parent_plugin_map(self):
        parent = self.legacy_linux_plan()
        original_plugins = parent.pop('plugins')
        plan = self.parallel_linux_plan(parent)
        plan['plugins'] = original_plugins
        with self.assertRaises(ValueError):
            blocks.declare(self.rebound_discovery_inputs(plan), self.design)

    def model_amendment_fixture(self, *, parallel=False):
        original = copy.deepcopy(self.spec)
        case = self.root / f'metadata-{self.serial}.case'
        case.write_text('synthetic immutable model bytes\n')
        sidecar = case.with_suffix('.json')
        original['panels']['B'][0].update(case_file=case.name,
            case_sha256=blocks.reference(case)['sha256'], absorbed_terms=2,
            categorical_intercepts=True)
        cell = original['panels']['B'][0]
        sidecar.write_text(json.dumps({'case_sha256': cell['case_sha256'],
                                      'body_sha256': cell['body_sha256'], 'absorbed_terms': 3}))
        parent = {**self.plan, 'tournament': routing.seal(original), 'frozen_files': {}}
        plan = self.parallel_linux_plan(parent) if parallel else parent
        inputs = self.rebound_discovery_inputs(plan, manifest=original)
        original_review = blocks.read(inputs['discovery_review'])
        amended = copy.deepcopy(original)
        amended['panels']['B'][0]['absorbed_terms'] = 3
        evidence = self.write({'independent_review': 'count correction before observations'})
        amendment = {'kind': blocks.KIND + '_B_model_metadata_amendment', 'status': 'passed',
            'calls': {'oracle': 0, 'native': 0, 'timing': 0},
            'original_manifest': inputs['manifest'],
            'original_manifest_sha256': routing.identity(original),
            'original_review': inputs['discovery_review'],
            'amended_manifest_sha256': routing.identity(amended),
            'changes': [{'cell_id': original['panels']['B'][0]['id'], 'before': 2, 'after': 3,
                         'case': blocks.reference(case), 'sidecar': blocks.reference(sidecar)}],
            'evidence': [evidence]}
        bound = [amendment['original_manifest'], amendment['original_review'], evidence,
                 amendment['changes'][0]['case'], amendment['changes'][0]['sidecar']]
        review = {**original_review, 'raw_evidence': original_review['raw_evidence'] + bound,
                  'model_metadata_amendment': amendment}
        return inputs, amended, review

    def test_reviewed_B_count_amendment_accepts_original_and_parallel_without_native_access(self):
        for parallel in [False, True]:
            with self.subTest(parallel=parallel):
                inputs, amended, review = self.model_amendment_fixture(parallel=parallel)
                inputs.update(manifest=self.write(routing.seal(amended)),
                              discovery_review=self.write(review))
                with patch.object(runner, 'inspect_case') as inspect:
                    context = blocks.declare(inputs, self.design)
                    inspect.assert_not_called()
                self.assertEqual(context['cells'][0]['absorbed_terms'], 3)
                self.assertEqual(context['cells'][0]['case_sha256'],
                    blocks.read(review['model_metadata_amendment']['original_manifest'],
                                sealed=True)['panels']['B'][0]['case_sha256'])

    def test_reviewed_B_count_amendment_rejects_unreviewed_changes_and_provenance(self):
        changes = ['A count', 'C count', 'body', 'policy', 'categorical', 'comparators',
                   'case identity', 'duplicate', 'missing evidence', 'missing raw proof',
                   'oracle calls', 'native calls', 'timing calls', 'bool before', 'bool after',
                   'extra manifest field', 'wrong original identity', 'wrong original review',
                   'missing original raw', 'case basename', 'sidecar path', 'unlisted change',
                   'original panel drift', 'zero before', 'zero after', 'unchanged count']
        for parallel in [False, True]:
            for change in changes:
                with self.subTest(parallel=parallel, change=change):
                    inputs, amended, review = self.model_amendment_fixture(parallel=parallel)
                    amendment = review['model_metadata_amendment']
                    correction = amendment['changes'][0]
                    cell = amended['panels']['B'][0]
                    if change in ['A count', 'C count']:
                        amended['panels'][change[0]][0]['absorbed_terms'] = 3
                    elif change == 'body':
                        cell['body_sha256'] = h('changed model bytes')
                    elif change == 'policy':
                        amended['arms']['h8-k0']['policy'] = routing.policy('h16-k0')
                    elif change == 'categorical':
                        cell['categorical_intercepts'] = False
                    elif change == 'comparators':
                        cell['comparators'] = []
                    elif change == 'case identity':
                        cell['case_sha256'] = h('changed input')
                    elif change == 'duplicate':
                        amendment['changes'].append(copy.deepcopy(correction))
                    elif change == 'missing evidence':
                        amendment['evidence'] = []
                    elif change == 'missing raw proof':
                        review['raw_evidence'].remove(correction['sidecar'])
                    elif change in ['oracle calls', 'native calls', 'timing calls']:
                        amendment['calls'][change.split()[0]] = 1
                    elif change == 'bool before':
                        correction['before'] = True
                    elif change == 'bool after':
                        correction['after'] = True
                        cell['absorbed_terms'] = True
                    elif change == 'extra manifest field':
                        amended['unreviewed_metadata'] = 'extra'
                    elif change == 'wrong original identity':
                        amendment['original_manifest_sha256'] = h('different original')
                    elif change == 'wrong original review':
                        previous = blocks.read(amendment['original_review'])
                        previous['remote_validation'] = self.write({'another': 'validation'})
                        replacement = self.write(previous)
                        review['raw_evidence'].append(replacement)
                        amendment['original_review'] = replacement
                    elif change == 'missing original raw':
                        previous = blocks.read(amendment['original_review'])
                        review['raw_evidence'].remove(previous['raw_evidence'][0])
                    elif change == 'case basename':
                        alternate = self.root / f'other-{self.serial}.case'
                        alternate.write_text('synthetic immutable model bytes\n')
                        correction['case'] = blocks.reference(alternate)
                        review['raw_evidence'].append(correction['case'])
                    elif change == 'sidecar path':
                        correction['sidecar'] = self.write({'wrong': 'sidecar location'})
                        review['raw_evidence'].append(correction['sidecar'])
                    elif change == 'original panel drift':
                        original = blocks.read(amendment['original_manifest'], sealed=True)
                        original['panels']['A'][0]['body_sha256'] = h('changed original A')
                        amendment['original_manifest'] = self.write(routing.seal(original))
                        amendment['original_manifest_sha256'] = routing.identity(original)
                        review['raw_evidence'].append(amendment['original_manifest'])
                        amended['panels']['A'] = copy.deepcopy(original['panels']['A'])
                    elif change == 'zero before':
                        correction['before'] = 0
                    elif change == 'zero after':
                        correction['after'] = cell['absorbed_terms'] = 0
                    elif change == 'unchanged count':
                        correction['after'] = cell['absorbed_terms'] = correction['before']
                    else:
                        amended['panels']['B'][1]['absorbed_terms'] = 3
                    # Rebind all affected outer objects: failures must exercise
                    # the amendment contract, not a stale amended-manifest seal.
                    amendment['amended_manifest_sha256'] = routing.identity(amended)
                    inputs.update(manifest=self.write(routing.seal(amended)),
                                  discovery_review=self.write(review))
                    with self.assertRaises(ValueError):
                        blocks.declare(inputs, self.design)

    def reject_original_review_raw_shape(self, shape):
        for parallel in [False, True]:
            with self.subTest(parallel=parallel):
                inputs, amended, review = self.model_amendment_fixture(parallel=parallel)
                amendment = review['model_metadata_amendment']
                previous = blocks.read(amendment['original_review'])
                if shape == 'missing':
                    del previous['raw_evidence']
                else:
                    previous['raw_evidence'] = [] if shape == 'empty' else 'not a list'
                replacement = self.write(previous)
                review['raw_evidence'].remove(amendment['original_review'])
                review['raw_evidence'].append(replacement)
                amendment['original_review'] = replacement
                inputs.update(manifest=self.write(routing.seal(amended)),
                              discovery_review=self.write(review))
                with self.assertRaisesRegex(ValueError, 'retain the original discovery review'):
                    blocks.declare(inputs, self.design)

    def test_model_amendment_rejects_empty_original_review_raw_evidence(self):
        self.reject_original_review_raw_shape('empty')

    def test_model_amendment_rejects_missing_original_review_raw_evidence(self):
        self.reject_original_review_raw_shape('missing')

    def test_model_amendment_rejects_nonlist_original_review_raw_evidence(self):
        self.reject_original_review_raw_shape('nonlist')

    def test_model_amendment_requires_unchanged_case_and_corrected_count_in_sidecar(self):
        for parallel in [False, True]:
            for field in ['case_sha256', 'body_sha256', 'absorbed_terms']:
                mutations = ['missing', 'forged']
                if field == 'absorbed_terms':
                    mutations.append('boolean')
                for mutation in mutations:
                    with self.subTest(parallel=parallel, field=field, mutation=mutation):
                        inputs, amended, review = self.model_amendment_fixture(parallel=parallel)
                        correction = review['model_metadata_amendment']['changes'][0]
                        previous = correction['sidecar']
                        metadata = blocks.read(previous)
                        if mutation == 'missing':
                            del metadata[field]
                        else:
                            metadata[field] = (True if mutation == 'boolean' else
                                               4 if field == 'absorbed_terms' else h('forged sidecar'))
                        Path(previous['path']).write_text(json.dumps(metadata))
                        correction['sidecar'] = blocks.reference(previous['path'])
                        review['raw_evidence'].remove(previous)
                        review['raw_evidence'].append(correction['sidecar'])
                        inputs.update(manifest=self.write(routing.seal(amended)),
                                      discovery_review=self.write(review))
                        with self.assertRaisesRegex(ValueError, 'sidecar does not attest the corrected count'):
                            blocks.declare(inputs, self.design)

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

    def test_platform_platform_Mac_spellings_admit_without_accepting_other_hosts(self):
        records = self.qualified()
        for value in ['macOS-26.6.2-arm64-arm-64bit', 'Darwin-25.6.0-arm64-arm-64bit']:
            with self.subTest(platform=value):
                blocks.qualification(self.context_ref, self.write({**records, 'platform': value}))
        for value in ['Linux-6.8-x86_64', 'Windows-11', 'Darwinian-host', 'macOSish', 'Darwin', '', None, {}, [], 1, True]:
            with self.subTest(platform=value), self.assertRaisesRegex(ValueError, 'fresh complete Mac'):
                blocks.qualification(self.context_ref, self.write({**records, 'platform': value}))

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
        proof = self.write(routing.seal({'synthetic': 'verified proof'}))
        with patch.object(runner, 'evidence', return_value=({'scoped_replay': proof}, {})):
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

    def test_ordinary_records_cannot_supply_derived_outcome_overrides(self):
        for field in ['effective_outcomes', 'effective_rows', 'completion', 'plan', 'kind']:
            records = self.qualified()
            records[field] = 'routing_science_completion_v1' if field == 'kind' else {'forged': 'passed'}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'separately validated'):
                blocks.qualification(self.context_ref, self.write(records))

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
        proof = self.write(routing.seal({'synthetic': 'verified proof'}))
        with patch.object(runner, 'evidence', return_value=({'scoped_replay': proof}, {})):
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
        proof = self.write(routing.seal({'synthetic': 'verified proof'}))
        with patch.object(runner, 'evidence', return_value=({'scoped_replay': proof}, {})), \
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
