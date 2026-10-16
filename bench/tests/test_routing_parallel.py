"""Parallel envelope: full producer contracts and fail-closed replacement rules."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_discovery as discovery
import routing_parallel as parallel
from test_routing import h
from test_routing_discovery import spec42, portfolio42, partition42, producer, save_json


class PortfolioParallelTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.spec = portfolio42()
        self.frozen = parallel.current_files()
        tests = discovery.reference(__file__)
        self.frozen[tests['path']] = tests['sha256']
        self.parent = {'kind': discovery.PORTFOLIO_KIND + '_plan', 'schema': 1, 'design': discovery.DESIGN,
                       'tournament': routing.seal(self.spec), 'producer_tools': {
                           p.name: sha for p, sha in parallel.routing_run.tool_identities().items()},
                       'plugins': {a: 'unused' for a in self.spec['arms']},
                       'library_limits': discovery.LIBRARY_LIMITS,
                       'output_root': str(self.root / 'fresh'), 'frozen_files': dict(self.frozen)}
        self.parent_ref = save_json(self.root / 'fresh-plan.json', routing.seal(self.parent))
        self.frozen[self.parent_ref['path']] = self.parent_ref['sha256']
        self.plan = {'kind': parallel.KIND + '_plan', 'schema': 2, 'design': copy.deepcopy(parallel.PORTFOLIO_DESIGN),
                     'parent': routing.seal(self.parent), 'parent_sha256': routing.identity(self.parent),
                     'parent_plan': self.parent_ref, 'producer_tools': self.parent['producer_tools'],
                     'receipt_generator': discovery.reference(parallel.isolation_host.__file__),
                     'frozen_files': self.frozen, 'output_root': str(self.root / 'closed')}
        self.evidence = patch.object(discovery.routing_run, 'evidence')
        self.evidence.start()
        self.addCleanup(self.evidence.stop)

    def node(self, n, hostname=None, job=None, shard=None):
        view = parallel.round_view(self.plan, n, 1, shard)
        spec = discovery.execution_spec(view)
        root = Path(view['output_root'])
        science = root / 'prerequisites/producer'
        science.mkdir(parents=True)
        original = producer(spec, True)
        original['tool_sha256'] = self.parent['producer_tools']
        machine = {'hostname': hostname or f'scc-node{n}' + (f'-shard{shard}' if shard else ''),
                   'job_id': job or str(100 + n * 2 + (shard or 0)),
                   'slots': 28, 'affinity': list(range(28)), 'machine': 'x86_64',
                   'library_limits': discovery.LIBRARY_LIMITS}
        proof = {'kind': discovery.KIND + '_prerequisites', 'status': 'ready',
                 'host': machine, 'plan_sha256': routing.identity(view), 'wrappers': {}}
        for index, cell in enumerate(spec['panels']['A']):
            proof['wrappers'][cell['id']] = save_json(science / f'witness-{index}.json', {'node': n, 'cell': cell['id']})
        timed = producer(spec)
        timed['tool_sha256'] = self.parent['producer_tools']
        for records in (original, timed):
            if shard is not None:
                records.update(discovery_partition=view['discovery_partition'],
                               parent_manifest_sha256=routing.identity(self.spec))
            for row in [*records['prerequisites'], *records['records']]:
                row['solver_rows'] = [[0.0] * 16, [0.0] * 16]
                if shard is not None:
                    family = next(c['family'] for c in spec['panels']['A'] if c['id'] == row['cell'])
                    row['model_discrete_sha256'] = h('scientific-model-' + family)
                row['science_sha256'] = h(f"host-{n}-{row['cell']}-{row['arm']}")
                if records is timed:
                    row['witness_sha256'] = proof['wrappers'][row['cell']]['sha256']
        proof['records'] = save_json(science / 'records.json', original)
        proof_ref = save_json(root / 'prerequisites/prerequisites.json', routing.seal(proof))
        output = root / 'collection'
        (output / 'round-1').mkdir(parents=True)
        record_ref = save_json(output / 'round-1/records.json', timed)
        collection = {'kind': discovery.KIND + '_collection', 'status': 'complete',
                      'plan_sha256': routing.identity(view), 'round_contract': view['round_contract'],
                      'prerequisites': proof_ref, 'prerequisites_sha256': routing.identity(proof),
                      'host': machine, 'rounds': [record_ref], 'producer_returncode': 0,
                      'supervisor_interrupted': False, 'observer_failure_seen': False,
                      'joint_started_monotonic_ns': 0, 'started_monotonic_ns': 100, 'completed_monotonic_ns': 300}
        save_json(output / 'collection.json', routing.seal(collection))
        (root / 'isolation').mkdir()
        save_json(root / 'isolation/receipt.json', routing.seal({'test': n}))
        save_json(root / 'isolation/accounting.json', {
            'argv': ['qacct', '-j', machine['job_id']], 'returncode': 0, 'locale': 'C', 'timezone': 'UTC',
            'stdout': 'ru_wallclock 1000.0\n'})
        (root / 'isolation/process_observations.jsonl').write_text(json.dumps({
            'kind': 'end', 'monotonic_ns': 400, 'collection_sha256': routing.identity(collection)}) + '\n')
        return view, collection, proof

    def reports(self):
        return [parallel.inspect_attempt(self.plan, None, n, 1) for n in (1, 2, 3)]

    def test_portable_metadata_and_current_source_closure(self):
        with patch.object(Path, 'open', side_effect=AssertionError('host path read')):
            self.assertEqual(parallel.metadata(self.plan)['spec'], self.spec)
        self.assertEqual(parallel.validate_plan(self.plan), self.spec)
        for key in ('proof', 'legacy_window', 'proof_sha256'):
            bad = {**self.plan, key: {}}
            with self.subTest(key=key), self.assertRaises(ValueError):
                parallel.metadata(bad)
        bad = copy.deepcopy(self.plan)
        bad['frozen_files'][str(Path(discovery.__file__).resolve())] = h('changed')
        with self.assertRaises(ValueError):
            parallel.validate_plan(bad)
        with self.assertRaises(ValueError):
            parallel.round_view(self.plan, 1, 2)

    def test_freeze_has_no_global_proof_or_legacy_window_and_binds_tests(self):
        source_check = self.root / 'checks.json'
        source_check.write_text('{"focused_tests":"passed"}')
        config = {'schema': 2, 'discovery_plan': self.parent_ref['path'],
                  'output_root': str(self.root / 'new-freeze'), 'receipt_generator': parallel.isolation_host.__file__,
                  'source_checks': [str(source_check)]}
        with patch.object(routing, 'scratch', side_effect=lambda p: p.resolve()):
            plan = parallel.freeze(config)
            self.assertEqual(plan['schema'], 2)
            self.assertNotIn('proof', plan)
            self.assertNotIn('legacy_window', plan)
            self.assertEqual(plan['frozen_files'][str(Path(__file__).resolve())], discovery.digest(Path(__file__)))
            with self.assertRaisesRegex(ValueError, 'no original proof or window'):
                parallel.freeze({**config, 'proof': 'old-proof.json'})

    def test_scheduler_recovery_is_explicit_frozen_portfolio_design_only(self):
        source_check = self.root / 'query-checks.json'
        source_check.write_text('{"focused_tests":"passed"}')
        config = {'schema': 2, 'discovery_plan': self.parent_ref['path'],
                  'output_root': str(self.root / 'query-freeze'), 'receipt_generator': parallel.isolation_host.__file__,
                  'source_checks': [str(source_check)]}
        with patch.object(routing, 'scratch', side_effect=lambda p: p.resolve()):
            original = parallel.freeze(config)
            self.assertEqual(original['design'], parallel.PORTFOLIO_DESIGN)
            self.assertNotIn('scheduler_query_policy', parallel.round_view(original, 1, 1))
            plan = parallel.freeze({**config, 'scheduler_query_policy': discovery.SCHEDULER_QUERY_POLICY['kind']})
            self.assertEqual(plan['design'], parallel.PORTFOLIO_QUERY_DESIGN)
            self.assertEqual(parallel.validate_plan(plan), self.spec)
            for n in (1, 2, 3):
                view = parallel.round_view(plan, n, 1)
                self.assertEqual(discovery.scheduler_query_policy(view), discovery.SCHEDULER_QUERY_POLICY)
                self.assertEqual(view['round_contract']['parallel_plan_sha256'], routing.identity(plan))
            for name in ('test_scc_routing_host.py', 'test_routing_discovery.py'):
                path = Path(__file__).with_name(name).resolve()
                self.assertEqual(plan['frozen_files'][str(path)], discovery.digest(path))
                bad = copy.deepcopy(plan)
                del bad['frozen_files'][str(path)]
                with self.assertRaises(ValueError):
                    parallel.validate_plan(bad)
            for policy in (None, 'unknown', discovery.SCHEDULER_QUERY_POLICY):
                with self.subTest(policy=policy), self.assertRaises(ValueError):
                    parallel.freeze({**config, 'scheduler_query_policy': policy})
        bad = copy.deepcopy(plan)
        bad['design']['scheduler_query_policy']['maximum_pair_attempts'] = 3
        with self.assertRaisesRegex(ValueError, 'wrong closed portfolio plan'):
            parallel.metadata(bad)
        with self.assertRaises(ValueError):
            parallel.round_view(plan, 1, 2)
        bad = copy.deepcopy(plan)
        parent = routing.unseal(bad['parent'])
        parent['scheduler_query_policy'] = copy.deepcopy(discovery.SCHEDULER_QUERY_POLICY)
        bad['parent'], bad['parent_sha256'] = routing.seal(parent), routing.identity(parent)
        with self.assertRaises(ValueError):
            parallel.metadata(bad)

    def test_producer_runs_own_two_call_science_then_one_full_round(self):
        view, _, proof = self.node(2)
        order = []
        def science(p, output):
            self.assertEqual(p, view)
            self.assertEqual(output, Path(view['output_root']) / 'prerequisites')
            order.append('science')
            return proof
        def invoke(p, spec, wrappers, output):
            self.assertEqual(p, view)
            self.assertEqual(spec, self.spec)
            self.assertEqual(len(wrappers), 42)
            self.assertEqual(output, Path(view['output_root']) / 'collection/round-1')
            order.append('round')
            return routing.read_json(output / 'records.json')
        with patch.object(discovery, 'prerequisites', side_effect=science), \
             patch.object(discovery, 'host', return_value=proof['host']), \
             patch.object(discovery, 'invoke', side_effect=invoke) as calls:
            parallel.producer(self.plan, None, 2, 1)
            self.assertEqual(order, ['science', 'round'])
            self.assertEqual(calls.call_count, 1)
            with self.assertRaisesRegex(ValueError, 'global prerequisite proof'):
                parallel.producer(self.plan, proof, 2, 1)
        report = discovery.load(Path(view['output_root']) / 'collection/producer-complete.json')
        self.assertEqual(report['proof'], discovery.reference(Path(view['output_root']) / 'prerequisites/prerequisites.json'))

    def test_joint_budget_stops_and_reaps_own_child_before_reserved_cleanup(self):
        view = parallel.round_view(self.plan, 1, 1)
        machine = {'hostname': 'scc-node1', 'job_id': '101', 'slots': 28, 'machine': 'x86_64',
                   'library_limits': discovery.LIBRARY_LIMITS}
        process = Mock()
        process.poll.return_value = None
        event = Mock()
        event.wait.return_value = False
        with patch.object(discovery, 'host', return_value=machine), \
             patch.object(parallel.subprocess, 'Popen', return_value=process), \
             patch.object(parallel, '_stop_owned', return_value={'returncode': -2, 'escalated': False}) as stop, \
             patch.object(parallel.time, 'monotonic_ns', return_value=14_400_000_000_000), \
             patch.object(parallel.signal, 'signal'):
            result = parallel._operation(self.plan, self.root / 'plan.json', 1, 1, view, None,
                                         Path(view['output_root']) / 'collection', event, joint_start_ns=0)
        self.assertEqual(result['status'], 'incomplete')
        self.assertIn('joint science/round budget expired', result['supervisor_error'])
        self.assertTrue(result['supervisor_interrupted'])
        stop.assert_called_once_with(process)

    def test_science_interruption_retains_real_partial_files_before_owned_cleanup(self):
        names = ('records.json', 'current.json', 'current.json.columns', 'current.log')
        for timing_started in (False, True):
            with self.subTest(timing_started=timing_started):
                self.plan['output_root'] = str(self.root / f'interrupted-{timing_started}')
                view = parallel.round_view(self.plan, 1, 1)
                root = Path(view['output_root'])
                sources = {'science': root / 'prerequisites/producer'}
                if timing_started:
                    sources['timing'] = root / 'collection/round-1'
                process = Mock()
                process.poll.return_value = None
                def launch(*_args, **_kwargs):
                    for phase, source in sources.items():
                        source.mkdir(parents=True)
                        for name in names:
                            (source / name).write_bytes(f'{phase}:{name}:partial-native-bytes'.encode())
                    return process
                def stop(child):
                    self.assertIs(child, process)
                    for phase, source in sources.items():
                        for name in names:
                            saved = root / 'interruption' / phase / name
                            self.assertEqual(saved.read_bytes(), (source / name).read_bytes())
                            (source / name).unlink()  # Model the producer's SIGINT finally cleanup.
                    return {'returncode': -2, 'escalated': False}
                event = Mock()
                event.wait.side_effect = KeyboardInterrupt('science interrupted')
                machine = {'hostname': 'scc-node1', 'job_id': '101', 'slots': 28, 'machine': 'x86_64',
                           'library_limits': discovery.LIBRARY_LIMITS}
                with patch.object(discovery, 'host', return_value=machine), \
                     patch.object(parallel.subprocess, 'Popen', side_effect=launch), \
                     patch.object(parallel, '_stop_owned', side_effect=stop), \
                     patch.object(parallel.time, 'monotonic_ns', return_value=100), \
                     patch.object(parallel.signal, 'signal'):
                    result = parallel._operation(self.plan, self.root / 'plan.json', 1, 1, view, None,
                                                 root / 'collection', event, joint_start_ns=0)
                self.assertEqual(result['status'], 'incomplete')
                self.assertEqual(set(result['interruption_artifacts']), {'science', 'timing'})
                for phase in sources:
                    for name in names:
                        ref = result['interruption_artifacts'][phase][name]
                        self.assertEqual(discovery.checked(ref).read_bytes(), f'{phase}:{name}:partial-native-bytes'.encode())
                if not timing_started:
                    self.assertEqual(result['interruption_artifacts']['timing'], {})

    def test_outer_alarm_during_owned_cleanup_is_deferred_after_early_term(self):
        import signal
        for early_term in (False, True):
            with self.subTest(early_term=early_term):
                self.plan['output_root'] = str(self.root / f'cleanup-alarm-{early_term}')
                view = parallel.round_view(self.plan, 1, 1)
                process, event = Mock(), Mock()
                process.poll.return_value = None
                def outer_alarm(signum, frame):
                    raise TimeoutError('outer entry deadline expired')
                prior = {signal.SIGTERM: signal.SIG_DFL, signal.SIGINT: signal.default_int_handler,
                         signal.SIGALRM: outer_alarm}
                handlers = dict(prior)
                def install(number, handler):
                    previous = handlers[number]
                    handlers[number] = handler
                    return previous
                def boundary(_timeout):
                    if early_term:
                        handlers[signal.SIGTERM](signal.SIGTERM, None)
                    raise KeyboardInterrupt('operation stopped before cleanup')
                event.wait.side_effect = boundary
                def stop(child):
                    self.assertIs(child, process)
                    handlers[signal.SIGALRM](signal.SIGALRM, None)
                    return {'returncode': -2, 'escalated': False}
                machine = {'hostname': 'scc-node1', 'job_id': '101', 'slots': 28, 'machine': 'x86_64',
                           'library_limits': discovery.LIBRARY_LIMITS}
                with patch.object(discovery, 'host', return_value=machine), \
                     patch.object(parallel.subprocess, 'Popen', return_value=process), \
                     patch.object(parallel, '_stop_owned', side_effect=stop) as cleanup, \
                     patch.object(parallel.time, 'monotonic_ns', return_value=100), \
                     patch.object(parallel.signal, 'getsignal', side_effect=lambda number: handlers[number]), \
                     patch.object(parallel.signal, 'signal', side_effect=install), \
                     patch.object(parallel.signal, 'setitimer') as timer:
                    result = parallel._operation(self.plan, self.root / 'plan.json', 1, 1, view, None,
                                                 Path(view['output_root']) / 'collection', event, joint_start_ns=0)
                cleanup.assert_called_once_with(process)
                timer.assert_not_called()
                self.assertEqual(handlers, prior)
                self.assertEqual(result['status'], 'incomplete')
                self.assertTrue(result['supervisor_interrupted'])
                self.assertEqual(result['producer_cleanup']['returncode'], -2)
                self.assertIn('interrupted during owned producer cleanup', result['supervisor_error'])
                self.assertEqual(discovery.load(Path(view['output_root']) / 'collection/collection.json'), result)

    def test_science_snapshot_failure_keeps_timing_capture_and_owned_cleanup(self):
        view = parallel.round_view(self.plan, 1, 1)
        root = Path(view['output_root'])
        process, event = Mock(), Mock()
        process.poll.return_value = None
        event.wait.side_effect = KeyboardInterrupt('interrupted')
        machine = {'hostname': 'scc-node1', 'job_id': '101', 'slots': 28, 'machine': 'x86_64',
                   'library_limits': discovery.LIBRARY_LIMITS}
        def snapshot(directory, saved):
            if directory == root / 'prerequisites/producer':
                raise OSError('science copy failed')
            self.assertEqual(directory, root / 'collection/round-1')
            self.assertEqual(saved, root / 'interruption/timing')
            return {'preserved': 'timing'}
        with patch.object(discovery, 'host', return_value=machine), \
             patch.object(parallel.subprocess, 'Popen', return_value=process), \
             patch.object(parallel, '_snapshot_partial', side_effect=snapshot), \
             patch.object(parallel, '_stop_owned', return_value={'returncode': -2, 'escalated': False}) as stop, \
             patch.object(parallel.time, 'monotonic_ns', return_value=100), \
             patch.object(parallel.signal, 'signal'):
            result = parallel._operation(self.plan, self.root / 'plan.json', 1, 1, view, None,
                                         root / 'collection', event, joint_start_ns=0)
        self.assertEqual(result['snapshot_errors'], {'science': 'science copy failed'})
        self.assertEqual(result['interruption_artifacts']['timing'], {'preserved': 'timing'})
        stop.assert_called_once_with(process)

    def test_three_local_proofs_allow_cross_cpu_bits_and_produce_nine_paired_slots(self):
        for n in (1, 2, 3):
            self.node(n)
        with patch.object(discovery, 'isolation') as isolation:
            reports = self.reports()
            result = parallel.aggregate(self.plan, None, reports)
        self.assertEqual(isolation.call_count, 6)
        self.assertEqual(result['schema'], 2)
        self.assertEqual(result['experiment_kind'], 'portfolio')
        self.assertFalse(result['promotion'])
        self.assertEqual(len(result['ranking']), 2)
        self.assertEqual(len(result['collection']['attempts']), 3)
        self.assertEqual(sum(r['accounted_wallclock_s'] for r in reports), 3000.0)
        self.assertEqual({r['science_discrete_sha256'] for r in reports}, {reports[0]['science_discrete_sha256']})
        with self.assertRaises(ValueError):
            parallel.aggregate(self.plan, {}, reports)
        with self.assertRaises(ValueError):
            parallel.aggregate(self.plan, None, reports[:2])
        with self.assertRaises(ValueError):
            parallel.aggregate(self.plan, None, reports[::-1])

    def test_shared_physical_host_or_job_and_replacement_output_refuse(self):
        for duplicate in ('host', 'job', 'replacement'):
            with self.subTest(duplicate=duplicate):
                # Each scenario gets an independent output tree.
                self.plan['output_root'] = str(self.root / duplicate)
                for n in (1, 2, 3):
                    self.node(n, hostname='scc-same' if duplicate == 'host' else None,
                              job='999' if duplicate == 'job' else None)
                if duplicate == 'replacement':
                    (Path(self.plan['output_root']) / 'round-2/attempt-2').mkdir()
                with patch.object(discovery, 'isolation'):
                    reports = self.reports()
                    with self.assertRaises(ValueError):
                        parallel.aggregate(self.plan, None, reports)

    def test_local_proof_host_round_and_budget_forgeries_refuse(self):
        for changed in ('host', 'proof', 'round', 'budget', 'observer', 'accounting'):
            with self.subTest(changed=changed):
                self.plan['output_root'] = str(self.root / changed)
                view, collection, proof = self.node(1)
                root = Path(view['output_root'])
                if changed == 'host':
                    collection['host'] = {**collection['host'], 'job_id': 'another-job'}
                elif changed == 'proof':
                    collection['prerequisites_sha256'] = h('another-proof')
                elif changed == 'round':
                    collection['rounds'] = []
                elif changed == 'observer':
                    collection['observer_failure_seen'] = True
                elif changed == 'accounting':
                    path = root / 'isolation/accounting.json'
                    value = routing.read_json(path)
                    value['stdout'] = 'ru_wallclock 14400.1\n'
                    save_json(path, value)
                else:
                    collection['joint_started_monotonic_ns'] = -15_000_000_000_000
                save_json(root / 'collection/collection.json', routing.seal(collection))
                with patch.object(discovery, 'isolation'), self.assertRaises(ValueError):
                    parallel.inspect_attempt(self.plan, None, 1, 1)

    def test_prospective_portfolio_screen_rejects_only_declared_point_failures(self):
        for violations, worst, rejected in [(0, 1.0, False), (0, 1.001, True), (1, .9, True), (2, 1.1, True)]:
            ranking = [{'arm': 'baseline'}, {'arm': routing.PORTFOLIO_V1,
                       'baseline_violations': violations, 'worst_internal_normalized': worst}]
            result = parallel.portfolio_screen(ranking)
            self.assertEqual(result['shortlist'] == ['baseline'], rejected)
            self.assertEqual(result['heuristic_rejection'] is not None, rejected)
        for worst in (None, float('nan'), float('inf'), True):
            with self.subTest(worst=worst), self.assertRaises(ValueError):
                parallel.portfolio_screen([{'arm': 'baseline'}, {'arm': routing.PORTFOLIO_V1,
                                           'baseline_violations': 0, 'worst_internal_normalized': worst}])

    def test_cross_cpu_continuous_pilot_contraction_and_iterations_are_allowed(self):
        original = producer(self.spec, True)
        expected = discovery.prerequisite_status(self.spec, original)
        for row in [*original['prerequisites'], *original['records']]:
            row['solver_rows'] = [[0.0] * 16, [0.0] * 16]
        first = parallel._discrete_science(original, expected)
        for row in [*original['prerequisites'], *original['records']]:
            row['solver_rows'][0][2] = 17.0
            row['solver_rows'][0][15] = .375
        self.assertEqual(parallel._discrete_science(original, expected), first)

    def test_cross_host_discrete_difference_and_failed_isolation_block_aggregation(self):
        for n in (1, 2, 3):
            self.node(n)
        with patch.object(discovery, 'isolation'):
            reports = self.reports()
            reports[1]['science_discrete_sha256'] = h('forged-summary')
            with self.assertRaises(ValueError):
                parallel.aggregate(self.plan, None, reports)
            # A real change retained in node 2's otherwise locally exact rows
            # also blocks, rather than merely rejecting a forged report field.
            view = parallel.round_view(self.plan, 2, 1)
            root = Path(view['output_root'])
            collection = discovery.load(root / 'collection/collection.json')
            proof = discovery.load(collection['prerequisites']['path'])
            for ref in (proof['records'], collection['rounds'][0]):
                value = routing.read_json(Path(ref['path']))
                for row in [*value['prerequisites'], *value['records']]:
                    row['solver_rows'][0][3] = 2.0
                ref.update(save_json(Path(ref['path']), value))
            proof_ref = save_json(root / 'prerequisites/prerequisites.json', routing.seal(proof))
            collection.update(prerequisites=proof_ref, prerequisites_sha256=routing.identity(proof))
            save_json(root / 'collection/collection.json', routing.seal(collection))
            changed = self.reports()
            self.assertNotEqual(changed[0]['science_discrete_sha256'], changed[1]['science_discrete_sha256'])
            with self.assertRaisesRegex(ValueError, 'cross-host discrete'):
                parallel.aggregate(self.plan, None, changed)
        with patch.object(discovery, 'isolation', side_effect=ValueError('accounting or coverage failure')):
            with self.assertRaises(ValueError):
                parallel.aggregate(self.plan, None, reports)


class ParallelTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.spec = spec42()
        self.parent = {'kind': discovery.KIND + '_plan', 'design': discovery.DESIGN,
                       'tournament': routing.seal(self.spec), 'producer_tools': {'test': h('source')},
                       'plugins': {a: 'unused' for a in self.spec['arms']},
                       'library_limits': discovery.LIBRARY_LIMITS,
                       'output_root': str(self.root / 'original'), 'frozen_files': {}}
        self.machine = {'hostname': 'test', 'job_id': '42', 'slots': 28, 'affinity': list(range(28)),
                        'machine': 'x86_64', 'library_limits': discovery.LIBRARY_LIMITS}
        self.original = producer(self.spec, True)
        self.proof = {'kind': discovery.KIND + '_prerequisites', 'status': 'ready',
                      'host': self.machine, 'plan_sha256': routing.identity(self.parent),
                      'records': save_json(self.root / 'original-records.json', self.original), 'wrappers': {}}
        for cell in self.spec['panels']['A']:
            cid = cell['id']
            self.proof['wrappers'][cid] = save_json(self.root / (cid + '-wrapper.json'), {'checked': cid})
        parent_ref = save_json(self.root / 'parent-plan.json', routing.seal(self.parent))
        proof_ref = save_json(self.root / 'proof.json', routing.seal(self.proof))
        host_path = self.root / 'host.py'
        host_path.write_text('# frozen host\n')
        host_ref = discovery.reference(host_path)
        self.plan = {'kind': parallel.KIND + '_plan', 'schema': 1, 'design': copy.deepcopy(parallel.DESIGN),
                     'parent': routing.seal(self.parent), 'parent_sha256': routing.identity(self.parent),
                     'parent_plan': parent_ref, 'proof': proof_ref, 'proof_sha256': routing.identity(self.proof),
                     'producer_tools': self.parent['producer_tools'], 'receipt_generator': host_ref,
                     'frozen_files': {r['path']: r['sha256'] for r in (parent_ref, proof_ref, host_ref)},
                     'output_root': str(self.root / 'new')}
        self.evidence = patch.object(discovery.routing_run, 'evidence')
        self.evidence.start()
        self.addCleanup(self.evidence.stop)

    def timed(self):
        value = producer(self.spec)
        for row in [*value['prerequisites'], *value['records']]:
            row['witness_sha256'] = self.proof['wrappers'][row['cell']]['sha256']
        return value

    def attempt(self, n, m=1, complete=True):
        view = parallel.round_view(self.plan, n, m)
        root = Path(view['output_root'])
        record_dir = root / 'collection/round-1'
        record_dir.mkdir(parents=True)
        record_ref = save_json(record_dir / 'records.json', self.timed())
        collection = {'kind': discovery.KIND + '_collection', 'plan_sha256': routing.identity(view),
                      'prerequisites_sha256': routing.identity(self.proof), 'round_contract': view['round_contract'],
                      'host': {**self.machine, 'job_id': str(100 * n + m)}, 'rounds': [record_ref],
                      'status': 'complete' if complete else 'incomplete', 'started_monotonic_ns': 100,
                      'completed_monotonic_ns': 300, 'supervisor_interrupted': not complete,
                      'interruption_artifacts': {}, 'producer_cleanup': {'returncode': -2, 'escalated': False}}
        collection_ref = save_json(root / 'collection/collection.json', routing.seal(collection))
        (root / 'isolation').mkdir()
        receipt_ref = save_json(root / 'isolation/receipt.json', routing.seal({'test': n}))
        report = {'kind': parallel.KIND + '_attempt', **view['round_contract'],
                  'collection': collection_ref, 'status': 'complete', 'isolation': receipt_ref}
        return view, collection, report

    def legacy_window(self):
        refs = []
        for n in (1, 2, 3):
            directory = Path(self.parent['output_root']) / f'collection/round-{n}'
            directory.mkdir(parents=True)
            refs.append(save_json(directory / 'records.json', self.timed()))
        collection = {'kind': discovery.KIND + '_collection', 'status': 'complete',
                      'plan_sha256': routing.identity(self.parent),
                      'prerequisites_sha256': routing.identity(self.proof),
                      'host': self.machine, 'rounds': refs}
        raw_path = self.root / 'legacy-raw.jsonl'
        raw_path.write_text('{"outside_window_error":"retained"}\n')
        raw = {key: discovery.reference(raw_path) for key in discovery.EVIDENCE}
        cert = {'kind': 'routing_scc_window_certificate', 'schema': 1, 'status': 'verified',
                'parent_sha256': routing.identity(self.parent), 'proof_sha256': routing.identity(self.proof),
                'collection_sha256': routing.identity(collection), 'collection': routing.seal(collection),
                'rounds': refs[:2], 'raw_evidence': raw, 'original_protocol_status': 'failed', 'promotion': False}
        cert_ref = save_json(self.root / 'window-certificate.json', routing.seal(cert))
        validator = self.root / 'window.py'
        validator.write_text('# frozen independent validator')
        validator_ref = discovery.reference(validator)
        self.plan['legacy_window'] = {'reference': cert_ref, 'certificate': routing.seal(cert),
                                      'validator': validator_ref}
        self.plan['frozen_files'].update({r['path']: r['sha256']
                                        for r in [cert_ref, validator_ref, *refs, *raw.values()]})
        return cert

    def test_metadata_is_portable_and_detects_parent_or_design_forgery(self):
        with patch.object(Path, 'open', side_effect=AssertionError('deployment file read')):
            self.assertEqual(parallel.metadata(self.plan)['parent'], self.parent)
        for field in ('parent_sha256', 'parent', 'design', 'producer_tools', 'frozen_files'):
            bad = copy.deepcopy(self.plan)
            if field == 'parent':
                bad[field]['value']['output_root'] += '-changed'
            elif field == 'design':
                bad[field]['maximum_attempts_per_round'] = 3
            elif field in ('producer_tools', 'frozen_files'):
                bad[field] = {}
            else:
                bad[field] = h('forged')
            with self.subTest(field=field), self.assertRaises(ValueError):
                parallel.metadata(bad)

    def test_round_views_preserve_full_parent_and_fixed_contract(self):
        for n in (1, 2, 3):
            view = parallel.round_view(self.plan, n, 1)
            self.assertEqual(view['tournament'], self.parent['tournament'])
            self.assertEqual(view['producer_tools'], self.parent['producer_tools'])
            self.assertEqual(len(routing.unseal(view['tournament'])['panels']['A']), 42)
            self.assertEqual(view['round_contract'], {'parallel_plan_sha256': routing.identity(self.plan),
                                                    'logical_round': n, 'attempt': 1})
        for n, m in ((True, 1), (4, 1), (1, True), (1, 3)):
            with self.assertRaises(ValueError):
                parallel.round_view(self.plan, n, m)

    def test_producer_is_one_unchanged_full_invocation_with_no_scoring(self):
        view = parallel.round_view(self.plan, 3, 1)
        calls = []
        def invoke(parent, spec, wrappers, output):
            self.assertEqual(parent, self.parent)
            self.assertEqual(spec, self.spec)
            self.assertEqual(set(wrappers), {c['id'] for c in self.spec['panels']['A']})
            self.assertEqual(output, Path(view['output_root']) / 'collection/round-1')
            output.mkdir(parents=True)
            value = self.timed()
            save_json(output / 'records.json', value)
            calls.append(output)
            return value
        with patch.object(parallel, 'validate_plan', return_value=self.spec), \
             patch.object(discovery, 'invoke', side_effect=invoke), \
             patch.object(discovery, 'score', side_effect=AssertionError('early timing selection')):
            value = parallel.producer(self.plan, self.proof, 3, 1)
        self.assertEqual(len(calls), 1)
        self.assertEqual(value['warmups'], 1)
        self.assertEqual(value['repetitions'], 3)
        self.assertEqual(value['timeout_s'], 120)
        self.assertEqual(value['budget_s'], 14400)

    def test_new_science_timeout_and_fingerprint_changes_cannot_be_infrastructure(self):
        view, collection, _ = self.attempt(1)
        root = Path(view['output_root'])
        (root / 'isolation/receipt.json').unlink()
        (root / 'isolation/scheduler.jsonl').write_text(json.dumps({
            'kind': 'error', 'monotonic_ns': 200, 'wall_time_ns': 1000,
            'error': 'external qstat command timeout'}) + '\n')
        for change in ('timeout', 'blocked', 'fingerprint', 'pending', 'duplicate'):
            raw = self.timed()
            row = raw['records'][0]
            if change == 'timeout':
                row.update(status='unavailable', failure={'kind': 'timeout'})
            elif change == 'blocked':
                row['status'] = 'blocked'
            elif change == 'fingerprint':
                row['science_sha256'] = h('changed')
            elif change == 'pending':
                row.pop('status')
                row.update(dispatched=True, process_completed=False)
            else:
                raw['records'].append(copy.deepcopy(row))
            records_path = root / 'collection/round-1/records.json'
            save_json(records_path, raw)
            collection['rounds'] = [discovery.reference(records_path)]
            save_json(root / 'collection/collection.json', routing.seal(collection))
            with self.subTest(change=change), patch.object(parallel, 'validate_plan', return_value=self.spec):
                self.assertEqual(parallel.inspect_attempt(self.plan, self.proof, 1, 1)['status'], 'failed')

    def test_infrastructure_requires_complete_science_and_raw_error(self):
        view, collection, _ = self.attempt(1)
        root = Path(view['output_root'])
        (root / 'isolation/receipt.json').unlink()
        with patch.object(parallel, 'validate_plan', return_value=self.spec):
            self.assertEqual(parallel.inspect_attempt(self.plan, self.proof, 1, 1)['status'], 'failed')
            error = {'kind': 'error', 'monotonic_ns': 200, 'wall_time_ns': 1000,
                     'error': "Command '['qstat', '-j', '101']' timed out after 10 seconds"}
            (root / 'isolation/scheduler.jsonl').write_text(json.dumps(error) + '\n')
            self.assertEqual(parallel.inspect_attempt(self.plan, self.proof, 1, 1)['status'], 'infra_failure')
            collection['status'] = 'incomplete'
            for code in (0, 430):
                current = root / 'collection/current.json'
                save_json(current, {'schema': 1, 'return_code': code, 'science_unchecked': True})
                collection['interruption_artifacts'] = {'current.json': discovery.reference(current)}
                save_json(root / 'collection/collection.json', routing.seal(collection))
                self.assertEqual(parallel.inspect_attempt(self.plan, self.proof, 1, 1)['status'], 'failed')

    def test_three_logical_rounds_concatenate_all_42_cells_to_nine_slots(self):
        reports = [self.attempt(n)[2] for n in (1, 2, 3)]
        by_label = {(r['logical_round'], r['attempt']): r for r in reports}
        original_score = discovery.score
        def score(spec, times, outcomes):
            self.assertEqual({c for c, _ in times}, {c['id'] for c in self.spec['panels']['A']})
            self.assertTrue(all(v is None or len(v) == 9 for v in times.values()))
            return original_score(spec, times, outcomes)
        with patch.object(parallel, 'validate_plan', return_value=self.spec), \
             patch.object(parallel, 'inspect_attempt', side_effect=lambda p, proof, n, m: by_label[n, m]), \
             patch.object(discovery, 'score', side_effect=score):
            out = parallel.aggregate(self.plan, self.proof, list(reversed(reports)))
            self.assertEqual(out['status'], 'discovery_only')
            self.assertFalse(out['promotion'])
            self.assertEqual([r['logical_round'] for r in out['collection']['selected']], [1, 2, 3])
            for bad in (reports[:2], reports + [reports[0]]):
                with self.assertRaises(ValueError):
                    parallel.aggregate(self.plan, self.proof, bad)
            first = discovery.load(reports[0]['collection']['path'])
            third_path = Path(reports[2]['collection']['path'])
            third = discovery.load(third_path)
            third['host']['job_id'] = first['host']['job_id']
            third_path.write_text(json.dumps(routing.seal(third)))
            reports[2]['collection'] = discovery.reference(third_path)
            with self.assertRaisesRegex(ValueError, 'distinct allocation'):
                parallel.aggregate(self.plan, self.proof, reports)

    def test_certified_windows_keep_original_identity_and_need_only_fresh_third(self):
        certificate = self.legacy_window()
        self.assertEqual(parallel.metadata(self.plan)['parent'], self.parent)
        for n in (1, 2):
            with self.assertRaisesRegex(ValueError, 'cannot be rerun'):
                parallel.round_view(self.plan, n, 1)
        _, _, report = self.attempt(3)
        with patch.object(parallel, 'validate_plan', return_value=self.spec), \
             patch.object(parallel, '_validate_window', return_value=certificate) as replay, \
             patch.object(parallel, 'inspect_attempt', return_value=report):
            result = parallel.aggregate(self.plan, self.proof, [report])
            replay.assert_called_once_with(self.plan, self.proof)
            self.assertEqual(result['collection']['legacy_window'], self.plan['legacy_window'])
            self.assertEqual(result['isolation'][0], self.plan['legacy_window']['reference'])
            self.assertEqual(result['status'], 'discovery_only')
            self.assertFalse(result['promotion'])
        with patch.object(parallel, 'validate_plan', return_value=self.spec), \
             patch.object(parallel, '_validate_window', side_effect=ValueError('bad raw window')), \
             patch.object(parallel, 'inspect_attempt') as inspect:
            with self.assertRaises(ValueError):
                parallel.aggregate(self.plan, self.proof, [report])
            inspect.assert_not_called()
        bad = copy.deepcopy(self.plan)
        bad['legacy_window']['certificate']['value']['parent_sha256'] = h('wrong parent')
        with self.assertRaises(ValueError):
            parallel.metadata(bad)

    def test_observer_fault_keeps_the_unchanged_producer_running_to_completion(self):
        from types import SimpleNamespace
        import signal
        view = parallel.round_view(self.plan, 1, 1)
        from unittest.mock import Mock
        process = SimpleNamespace(pid=987654, poll=Mock(side_effect=[None, 0, 0]),
                                  wait=lambda timeout=None: 0)
        event = SimpleNamespace(wait=lambda seconds: True)
        output = Path(view['output_root']) / 'collection'
        with patch.object(discovery, 'host', return_value=self.machine), \
             patch.object(parallel.subprocess, 'Popen', return_value=process) as popen, \
             patch.object(parallel.os, 'killpg') as kill:
            result = parallel._operation(self.plan, self.root / 'frozen-plan.json', 1, 1,
                                         view, self.proof, output, event)
        self.assertTrue(popen.call_args.kwargs['start_new_session'])
        self.assertEqual(popen.call_args.args[0][-4:], ['--round', '1', '--attempt', '1'])
        kill.assert_not_called()
        self.assertFalse(result['supervisor_interrupted'])
        self.assertTrue(result['observer_failure_seen'])
        self.assertEqual(result['status'], 'incomplete')
        self.assertTrue((output / 'producer.log').exists())
        self.assertTrue((output / 'collection.json').exists())

    def test_snapshot_error_and_supervisor_interrupt_still_reap_owned_producer(self):
        from types import SimpleNamespace
        for failure in (OSError('snapshot copy failed'), ValueError('snapshot malformed')):
            with self.subTest(failure=type(failure).__name__):
                attempt = 1 if isinstance(failure, OSError) else 2
                view = parallel.round_view(self.plan, 1, attempt)
                process = SimpleNamespace(pid=987654, poll=lambda: None, wait=lambda timeout=None: -2)
                from unittest.mock import Mock
                event = SimpleNamespace(wait=Mock(side_effect=KeyboardInterrupt('supervisor interrupted')))
                output = Path(view['output_root']) / 'collection'
                with patch.object(discovery, 'host', return_value=self.machine), \
                     patch.object(parallel.subprocess, 'Popen', return_value=process), \
                     patch.object(parallel, '_snapshot_partial', side_effect=failure), \
                     patch.object(parallel.os, 'killpg', side_effect=ProcessLookupError) as kill:
                    result = parallel._operation(self.plan, self.root / 'plan', 1, attempt,
                                                 view, self.proof, output, event)
                kill.assert_called_once()
                self.assertIn('supervisor_error', result)
                self.assertEqual(result['producer_cleanup']['returncode'], -2)
                self.assertTrue((output / 'collection.json').exists())

    def test_cleanup_escalation_signals_only_captured_matching_owned_child(self):
        from types import SimpleNamespace
        from unittest.mock import Mock
        import signal
        import subprocess
        root = {'pid': 100, 'ppid': 1, 'start': 20}
        child = {'pid': 101, 'ppid': 100, 'start': 21}
        process = SimpleNamespace(pid=100, poll=lambda: None, kill=Mock(),
                                  wait=Mock(side_effect=[subprocess.TimeoutExpired('producer', 30), -9]))
        with patch.object(parallel, '_owned_descendants', return_value={100: root, 101: child}), \
             patch.object(parallel, '_proc', return_value=child), \
             patch.object(parallel.os, 'killpg') as group, \
             patch.object(parallel.os, 'kill') as kill:
            result = parallel._stop_owned(process)
        group.assert_called_once_with(100, signal.SIGINT)
        kill.assert_called_once_with(101, signal.SIGKILL)
        process.kill.assert_called_once()
        self.assertTrue(result['escalated'])
        self.assertEqual(process.wait.call_args.kwargs['timeout'], 10)

    def test_pending_call_without_completed_science_or_timeout_boundary_is_not_retryable(self):
        view, collection, _ = self.attempt(1, complete=False)
        root = Path(view['output_root'])
        records_path = root / 'collection/round-1/records.json'
        raw = self.timed()
        raw['records'] = raw['records'][:1]
        row = raw['records'][0]
        row.pop('status')
        row.update(dispatched=True, process_completed=False)
        save_json(records_path, raw)
        collection['rounds'] = [discovery.reference(records_path)]
        save_json(root / 'collection/collection.json', routing.seal(collection))
        (root / 'isolation/scheduler.jsonl').write_text(json.dumps({
            'kind': 'error', 'monotonic_ns': 200, 'wall_time_ns': 1000,
            'error': 'external qstat command timeout'}) + '\n')
        with patch.object(parallel, 'validate_plan', return_value=self.spec):
            self.assertEqual(parallel.inspect_attempt(self.plan, self.proof, 1, 1)['status'], 'failed')

    def test_finalized_infrastructure_report_roundtrips_through_inspection_and_aggregate(self):
        view, collection, _ = self.attempt(1)
        root = Path(view['output_root'])
        (root / 'isolation/receipt.json').unlink()
        (root / 'isolation/scheduler.jsonl').write_text(json.dumps({
            'kind': 'error', 'monotonic_ns': 200, 'wall_time_ns': 1000,
            'error': 'external qstat command timeout'}) + '\n')
        account = root / 'isolation/accounting.json'
        account.write_text('{"captured":"terminal accounting"}\n')
        original_bytes = account.read_bytes()
        for n, m in ((1, 2), (2, 1), (3, 1)):
            self.attempt(n, m)
        with patch.object(parallel, 'validate_plan', return_value=self.spec), \
             patch.object(parallel.isolation_host, 'accounting', return_value={'failed': 0, 'exit_status': 1}), \
             patch.object(parallel.isolation_host, 'derived', side_effect=ValueError('observer error')), \
             patch.object(discovery, 'isolation'):
            finalized = parallel.finalize(self.plan, self.proof, 1, 1)
            self.assertEqual(finalized['status'], 'infra_failure')
            self.assertIn('accounting', finalized)
            self.assertEqual(finalized, parallel.inspect_attempt(self.plan, self.proof, 1, 1))
            attempts = [finalized, *[parallel.inspect_attempt(self.plan, self.proof, n, m)
                                    for n, m in ((1, 2), (2, 1), (3, 1))]]
            self.assertEqual(parallel.aggregate(self.plan, self.proof, attempts)['status'], 'discovery_only')
            self.assertEqual(account.read_bytes(), original_bytes)

    def test_finalize_recovers_existing_accounting_and_receipt_without_overwriting(self):
        view, _, _ = self.attempt(1)
        root = Path(view['output_root'])
        receipt = root / 'isolation/receipt.json'
        receipt.unlink()
        for key in discovery.EVIDENCE:
            suffix = '.json' if key == 'accounting' else '.jsonl'
            (root / 'isolation' / (key + suffix)).write_text('{"captured":"evidence"}\n')
        account = root / 'isolation/accounting.json'
        original = account.read_bytes()
        with patch.object(parallel, 'validate_plan', return_value=self.spec), \
             patch.object(parallel.isolation_host, 'accounting', return_value={'failed': 0, 'exit_status': 0}), \
             patch.object(parallel.isolation_host, 'derived', return_value={'test': 'derived facts'}), \
             patch.object(parallel.isolation_host, 'validate'), \
             patch.object(parallel.isolation_host, 'finalize') as capture, \
             patch.object(discovery, 'isolation'):
            first = parallel.finalize(self.plan, self.proof, 1, 1)
            receipt_bytes = receipt.read_bytes()
            self.assertEqual(first['status'], 'complete')
            second = parallel.finalize(self.plan, self.proof, 1, 1)
            self.assertEqual(first, second)
            self.assertEqual(account.read_bytes(), original)
            self.assertEqual(receipt.read_bytes(), receipt_bytes)
            capture.assert_not_called()

    def test_descendant_inspection_error_cannot_skip_killing_and_reaping_owned_producer(self):
        from types import SimpleNamespace
        from unittest.mock import Mock
        import subprocess
        root = {'pid': 100, 'ppid': 1, 'start': 20}
        child = {'pid': 101, 'ppid': 100, 'start': 21}
        process = SimpleNamespace(pid=100, poll=lambda: None, kill=Mock(),
                                  wait=Mock(side_effect=[subprocess.TimeoutExpired('producer', 30), -9]))
        with patch.object(parallel, '_owned_descendants', return_value={100: root, 101: child}), \
             patch.object(parallel, '_proc', side_effect=OSError('inspection failed')), \
             patch.object(parallel.os, 'killpg'):
            with self.assertRaises(OSError):
                parallel._stop_owned(process)
        process.kill.assert_called_once()
        self.assertEqual(process.wait.call_count, 2)
        self.assertEqual(process.wait.call_args.kwargs['timeout'], 10)

    def test_replacement_requires_first_attempt_and_rejects_timing_choice(self):
        reports = [self.attempt(n)[2] for n in (1, 2, 3)]
        second = self.attempt(1, 2)[2]
        labels = {(r['logical_round'], r['attempt']): r for r in [*reports, second]}
        with patch.object(parallel, 'validate_plan', return_value=self.spec), \
             patch.object(parallel, 'inspect_attempt', side_effect=lambda p, proof, n, m: labels[n, m]):
            with self.assertRaisesRegex(ValueError, 'cannot be replaced'):
                parallel.aggregate(self.plan, self.proof, [*reports, second])
            with self.assertRaisesRegex(ValueError, 'attempt one'):
                parallel.aggregate(self.plan, self.proof, [second, *reports[1:]])
            reports[0]['status'] = 'failed'
            with self.assertRaises(ValueError):
                parallel.aggregate(self.plan, self.proof, [*reports, second])
            reports[0]['status'] = 'infra_failure'
            reports[0]['accounting'] = reports[0].pop('isolation')
            result = parallel.aggregate(self.plan, self.proof, [*reports, second])
            self.assertEqual(result['collection']['selected'][0]['attempt'], 2)
            self.assertEqual(len(result['collection']['attempts']), 4)


class PartitionParallelTests(unittest.TestCase):
    node = PortfolioParallelTests.node

    def setUp(self):
        PortfolioParallelTests.setUp(self)
        self.spec = partition42()
        self.parent['tournament'] = routing.seal(self.spec)
        self.parent['plugins'] = {a: 'unused' for a in self.spec['arms']}
        for name in ('test_routing_discovery.py', 'test_scc_routing_host.py'):
            ref = discovery.reference(Path(__file__).with_name(name))
            self.frozen[ref['path']] = ref['sha256']
        self.parent['frozen_files'] = dict(self.frozen)
        self.parent_ref = save_json(self.root / 'partition-parent.json', routing.seal(self.parent))
        self.frozen[self.parent_ref['path']] = self.parent_ref['sha256']
        self.plan.update(schema=3, design=copy.deepcopy(parallel.PARTITION_DESIGN),
                         parent=routing.seal(self.parent), parent_plan=self.parent_ref,
                         parent_sha256=routing.identity(self.parent), partitions=discovery.partitions(self.spec))

    def six_nodes(self):
        for n in (1, 2, 3):
            for shard in (1, 2):
                self.node(n, shard=shard)
        return [parallel.inspect_attempt(self.plan, None, n, 1, shard) for n in (1, 2, 3) for shard in (1, 2)]

    def test_portable_full_parent_schema_and_strict_shard_dispatch(self):
        with patch.object(Path, 'open', side_effect=AssertionError('host file read')):
            self.assertEqual(parallel.metadata(self.plan)['spec'], self.spec)
        self.assertEqual(parallel.validate_plan(self.plan), self.spec)
        for shard, count in [(1, 20), (2, 22)]:
            view = parallel.round_view(self.plan, 2, 1, shard)
            self.assertEqual(Path(view['output_root']).relative_to(self.plan['output_root']).as_posix(),
                             f'round-2/shard-{shard}/attempt-1')
            self.assertEqual(len(discovery.execution_spec(view)['panels']['A']), count)
            self.assertEqual(discovery.scheduler_query_policy(view), discovery.SCHEDULER_QUERY_POLICY)
        for shard, attempt in [(None, 1), (True, 1), (3, 1), (1, 2)]:
            with self.subTest(shard=shard, attempt=attempt), self.assertRaises(ValueError):
                parallel.round_view(self.plan, 1, attempt, shard)
        for change in ('schema', 'design', 'partitions', 'candidate'):
            bad = copy.deepcopy(self.plan)
            if change == 'schema': bad['schema'] = 2
            elif change == 'design': bad['design'] = parallel.PORTFOLIO_QUERY_DESIGN
            elif change == 'partitions': bad['partitions'][0]['inclusive_allocation_budget_s'] += 1
            else:
                parent = routing.unseal(bad['parent']); parent['tournament'] = routing.seal(portfolio42())
                bad.update(parent=routing.seal(parent), parent_sha256=routing.identity(parent))
            with self.subTest(change=change), self.assertRaises(ValueError): parallel.metadata(bad)

    def test_freeze_preserves_full_parent_and_binds_all_partition_tools(self):
        check = self.root / 'checks.txt'; check.write_text('passed')
        config = {'schema': 3, 'discovery_plan': self.parent_ref['path'],
                  'output_root': str(self.root / 'partitioned-freeze'),
                  'receipt_generator': parallel.isolation_host.__file__, 'source_checks': [str(check)],
                  'scheduler_query_policy': discovery.SCHEDULER_QUERY_POLICY['kind']}
        with patch.object(routing, 'scratch', side_effect=lambda p: p.resolve()):
            frozen = parallel.freeze(config)
            self.assertEqual(frozen['partitions'], discovery.partitions(self.spec))
            self.assertEqual(parallel.metadata(frozen)['spec'], self.spec)
            missing = dict(config); del missing['scheduler_query_policy']
            with self.assertRaises(ValueError): parallel.freeze(missing)

    def test_all_six_raw_proofs_before_full_parent_score(self):
        with patch.object(discovery, 'isolation') as replay:
            reports = self.six_nodes()
            with patch.object(discovery, 'score', wraps=discovery.score) as score:
                result = parallel.aggregate(self.plan, None, reports)
                score.assert_called_once()
                self.assertEqual(score.call_args.args[0], self.spec)
                self.assertEqual(len(score.call_args.args[1]), 360)
                self.assertTrue(all(len(v) == 9 for v in score.call_args.args[1].values()))
        self.assertEqual(replay.call_count, 12)
        self.assertEqual(result['schema'], 3)
        self.assertFalse(result['promotion'])
        self.assertEqual(len(result['collection']['attempts']), 6)
        with patch.object(discovery, 'score') as score:
            for bad in (reports[:5], reports[::-1], reports[:3] * 2):
                with self.assertRaises(ValueError): parallel.aggregate(self.plan, None, bad)
            score.assert_not_called()

    def test_duplicate_host_or_replacement_and_forged_report_block_global_score(self):
        self.node(1, shard=1)
        self.node(1, shard=2, hostname='scc-node1-shard1')
        for n in (2, 3):
            for s in (1, 2): self.node(n, shard=s)
        with patch.object(discovery, 'isolation'), patch.object(discovery, 'score') as score:
            reports = [parallel.inspect_attempt(self.plan, None, n, 1, s) for n in (1, 2, 3) for s in (1, 2)]
            with self.assertRaisesRegex(ValueError, 'six distinct'): parallel.aggregate(self.plan, None, reports)
            bad = copy.deepcopy(reports); bad[0]['accounted_wallclock_s'] += 1
            with self.assertRaisesRegex(ValueError, 'raw replay'): parallel.aggregate(self.plan, None, bad)
            (Path(self.plan['output_root']) / 'round-1/shard-1/attempt-2').mkdir()
            with self.assertRaisesRegex(ValueError, 'replacements'): parallel.aggregate(self.plan, None, reports)
            score.assert_not_called()

    def test_subset_science_identity_and_budget_cannot_import_other_shard(self):
        view, collection, proof = self.node(1, shard=1)
        original = routing.read_json(discovery.checked(proof['records']))
        discovery.validate_prerequisite_identity(view, original)
        for field, value in [('manifest_sha256', routing.identity(self.spec)),
                             ('parent_manifest_sha256', h('other-parent')),
                             ('discovery_partition', self.plan['partitions'][1]), ('budget_s', 14400)]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                discovery.validate_prerequisite_identity(view, {**original, field: value})
        account = Path(view['output_root']) / 'isolation/accounting.json'
        value = routing.read_json(account)
        value['stdout'] = f"ru_wallclock {self.plan['partitions'][0]['inclusive_allocation_budget_s'] + 1}\n"
        account.write_text(json.dumps(value))
        with patch.object(discovery, 'isolation'), self.assertRaisesRegex(ValueError, 'inclusive allocation'):
            parallel.inspect_attempt(self.plan, None, 1, 1, 1)

    def test_real_cross_host_decision_change_and_failed_receipt_block_global_score(self):
        with patch.object(discovery, 'isolation'):
            reports = self.six_nodes()
            view = parallel.round_view(self.plan, 2, 1, 1)
            root = Path(view['output_root'])
            collection = discovery.load(root / 'collection/collection.json')
            proof = discovery.load(collection['prerequisites']['path'])
            for ref in (proof['records'], collection['rounds'][0]):
                value = routing.read_json(Path(ref['path']))
                for row in [*value['prerequisites'], *value['records']]:
                    row['solver_rows'][0][3] = 2.0
                ref.update(save_json(Path(ref['path']), value))
            proof_ref = save_json(root / 'prerequisites/prerequisites.json', routing.seal(proof))
            collection.update(prerequisites=proof_ref, prerequisites_sha256=routing.identity(proof))
            save_json(root / 'collection/collection.json', routing.seal(collection))
            changed = [parallel.inspect_attempt(self.plan, None, n, 1, s) for n in (1, 2, 3) for s in (1, 2)]
            self.assertNotEqual(changed[0]['science_discrete_sha256'], changed[2]['science_discrete_sha256'])
            with patch.object(discovery, 'score') as score, self.assertRaisesRegex(ValueError, 'cross-host subset discrete'):
                parallel.aggregate(self.plan, None, changed)
            score.assert_not_called()
        with patch.object(discovery, 'isolation', side_effect=ValueError('isolation failed')):
            with self.assertRaisesRegex(ValueError, 'isolation failed'):
                parallel.aggregate(self.plan, None, reports)

    def test_model_decisions_exact_across_workers_but_resource_routes_are_descriptive(self):
        view, _, proof = self.node(1, shard=1)
        spec = discovery.execution_spec(view)
        raw = routing.read_json(discovery.checked(proof['records']))
        outcomes = discovery.prerequisite_status(spec, raw)
        original = parallel._partition_science(spec, raw, outcomes)
        self.assertEqual(original['worker_route_differences'], [])
        cid = next(c['id'] for c in spec['panels']['A'] if c['threads'] == 4)
        for row in [*raw['prerequisites'], *raw['records']]:
            if row['cell'] == cid and row['arm'] == routing.PORTFOLIO_UNSEEDED:
                row['solver_rows'][0][1] = 3.0
                row['solver_rows'][0][11] = 7.0
        changed = parallel._partition_science(spec, raw, outcomes)
        self.assertEqual(changed['model_discrete_sha256'], original['model_discrete_sha256'])
        self.assertEqual(len(changed['worker_route_differences']), 1)
        for row in [*raw['prerequisites'], *raw['records']]:
            if row['cell'] == cid:
                row['model_discrete_sha256'] = h('changed-model')
        with self.assertRaisesRegex(ValueError, 'cross-worker scientific'):
            parallel._partition_science(spec, raw, outcomes)
        del raw['prerequisites'][0]['model_discrete_sha256']
        with self.assertRaisesRegex(ValueError, 'missing or changed'):
            parallel._partition_science(spec, raw, outcomes)

    def test_supervisor_uses_shard_budget_cleanup_reserve_and_explicit_command(self):
        view = parallel.round_view(self.plan, 1, 1, 1)
        budget = view['discovery_partition']['inclusive_allocation_budget_s']
        machine = {'hostname': 'node', 'job_id': '9', 'slots': 28, 'machine': 'x86_64',
                   'library_limits': discovery.LIBRARY_LIMITS}
        process, event = Mock(), Mock(); process.poll.return_value = None
        with patch.object(discovery, 'host', return_value=machine), \
             patch.object(parallel.subprocess, 'Popen', return_value=process) as launch, \
             patch.object(parallel, '_stop_owned', return_value={'returncode': -2, 'escalated': False}) as stop, \
             patch.object(parallel.time, 'monotonic_ns', return_value=int((budget - 180) * 1e9)), \
             patch.object(parallel.signal, 'signal'):
            result = parallel._operation(self.plan, self.root / 'plan.json', 1, 1, view, None,
                                         Path(view['output_root']) / 'collection', event, joint_start_ns=0)
        self.assertEqual(launch.call_args.args[0][-2:], ['--shard', '1'])
        self.assertIn('joint science/round budget expired', result['supervisor_error'])
        stop.assert_called_once_with(process)


if __name__ == '__main__':
    unittest.main()
