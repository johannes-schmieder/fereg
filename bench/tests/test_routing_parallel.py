"""Parallel envelope: full producer contracts and fail-closed replacement rules."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_discovery as discovery
import routing_parallel as parallel
from test_routing import h
from test_routing_discovery import spec42, producer, save_json


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


if __name__ == '__main__':
    unittest.main()
