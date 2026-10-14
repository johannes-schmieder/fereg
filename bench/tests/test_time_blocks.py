"""Paired-block transport/statistics tests; every native solve is synthetic."""
import copy
from fractions import Fraction
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import time_blocks as blocks


def bits(value):
    return struct.pack('>d', value).hex()


def raw_result():
    shapes = {'RREC': (1, 16), 'RCERT': (32, 2), 'RSOLVER': (2, 16),
              'RRUNTIME': (30, 2), 'RMEMORY': (11, 2), 'RB': (1, 1)}
    return {'return_code': 0, 'columns': [], 'error': '', 'matrices': {
        name: {'rows': r, 'columns': c, 'bits': [bits(1.)] * (r*c)} for name, (r, c) in shapes.items()}}


def scoring_block(reference, candidate, order='ABBA'):
    values = {'A': iter(reference), 'B': iter(candidate)}
    sides = ['reference' if letter == 'A' else 'candidate' for letter in order]
    return {'status': 'complete', 'sides': sides, 'calls': [
        {'side': side, 'seconds': next(values[letter]), 'status': 'passed', 'science_pass': True,
         'certificate_pass': True} for letter, side in zip(order, sides)]}


class Statistics(unittest.TestCase):
    def test_order_statistic_rank_and_exact_binomial_tail(self):
        result = blocks.median_interval(list(range(12)))
        self.assertEqual(result['upper_rank_1based'], 10)
        self.assertEqual((result['lower'], result['upper']), (2, 9))
        self.assertEqual(result['attained_upper_tail'], {'numerator': 79, 'denominator': 4096})
        for n in range(1, 45):
            result = blocks.median_interval(list(range(n)))
            rank = result['upper_rank_1based']
            if rank:
                tail = sum(math.comb(n, k) for k in range(rank, n+1))
                self.assertLessEqual(Fraction(tail, 2**n), Fraction(1, 40))
                self.assertGreater(Fraction(tail + math.comb(n, rank-1), 2**n), Fraction(1, 40))
            else:
                self.assertGreater(Fraction(1, 2**n), Fraction(1, 40))

    def test_no_attainable_rank_and_ties_are_not_dropped(self):
        self.assertTrue(blocks.median_interval([0.] * 5)['upper_unbounded'])
        result = blocks.median_interval([0.] * 6)
        self.assertEqual((result['upper'], result['n']), (0., 6))
        for bad in [[], [float('nan')], [float('inf')]]:
            with self.assertRaises(ValueError): blocks.median_interval(bad)

    def test_geometric_means_and_measured_baseline_allowance(self):
        score = blocks.block_scores(scoring_block([1., 4.], [2., 8.]))
        self.assertAlmostEqual(score['reference'], 2.)
        self.assertAlmostEqual(score['candidate'], 4.)
        self.assertAlmostEqual(score['ratio'], 2.)
        self.assertAlmostEqual(score['excess_seconds'], 1.9)
        score = blocks.block_scores(scoring_block([.002, .002], [.0025, .0025], 'BAAB'))
        self.assertAlmostEqual(score['excess_seconds'], -.0005)
        self.assertGreater(score['ratio'], 1.05)  # absolute allowance is tested inside each block

    def test_failed_reordered_nonfinite_calls_are_not_scores(self):
        for mutate in [lambda b: b['calls'].pop(), lambda b: b['calls'][0].update(status='failed'),
                       lambda b: b['calls'][0].update(science_pass=False),
                       lambda b: b['calls'][0].update(seconds=float('nan')),
                       lambda b: b['calls'][0].update(side='candidate')]:
            block = scoring_block([1., 1.], [1., 1.])
            mutate(block)
            with self.assertRaises(ValueError): blocks.block_scores(block)


class FrozenCollection(unittest.TestCase):
    def setUp(self):
        parent = blocks.ROOT / 'target/tmp/solver-routing'
        parent.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix='block-tool-tests-', dir=parent)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.raw = raw_result()
        self.columns = struct.pack('=12d', *range(12))
        for name in ['plugin', 'harness', 'declaration']:
            (self.root / name).write_text(name)
        args = [b'estimate-ols', b'threads=1']
        (self.root / 'input.case').write_bytes(b'FEREG_REPLAY_V1\n2 6 2 1 0\n' + b''.join(
            str(len(a)).encode() + b'\n' + a + b'\n' for a in args) + b'body\n')
        # Use the actual maintained exact-view function and binary-column hash.
        import native_io
        exact = native_io.exact_view(self.raw, False)
        exact['columns_sha256'] = blocks.hashlib.sha256(self.columns).hexdigest()
        self.fingerprint = blocks.identity(exact)
        qualification = {'status': 'complete', 'all_exact': True, 'exactness': 'all plugins',
                         'warmups': 1, 'repetitions': 1,
                         'cases': {str(self.root / 'input.case'): blocks.digest(self.root / 'input.case')},
                         'plugins': {'old': blocks.reference(self.root / 'plugin'), 'new': blocks.reference(self.root / 'plugin')},
                         'records': [{'case': 'input.case', 'plugin': name, 'repetition': rep,
                                      'science_sha256': self.fingerprint} for name in ['old', 'new'] for rep in [-1, 0]]}
        (self.root / 'qualification.json').write_text(json.dumps(qualification))
        arm = {'plugin': blocks.reference(self.root / 'plugin'), 'input': blocks.reference(self.root / 'input.case')}
        self.spec = {'schema': 1, 'harness': blocks.reference(self.root / 'harness'),
                     'design': {'attempts': 6, 'warmups': 1, 'seed': 193, 'timeout_s': 90, 'budget_s': 900,
                                'session_id': 'synthetic-session', 'analysis_unit': 'block',
                                'alpha': {'numerator': 1, 'denominator': 40}},
                     'comparisons': [{'id': 'case', 'workload': 'case',
                                      'reference': {**arm, 'label': 'A'}, 'candidate': {**arm, 'label': 'B'},
                                      'validation': {'kind': 'exact', 'poisson': False,
                                                     'qualification': blocks.reference(self.root / 'qualification.json')}}],
                     'monitor': None, 'declarations': [blocks.reference(self.root / 'declaration')]}
        self.manifest = self.root / 'manifest.json'
        self.out = self.root / 'collection'
        # Keep synthetic tests fast; the production function has its own inventory assertion.
        self.patcher = patch.object(blocks, 'source_bindings', return_value=[blocks.reference(Path(blocks.__file__))])
        self.patcher.start()
        self.addCleanup(self.patcher.stop)
        self.calls = 0

    def freeze(self):
        blocks.freeze(self.spec, self.manifest)

    def execute(self, command, log, timeout, started):
        self.calls += 1
        self.assertGreater(timeout, 0)
        self.assertLessEqual(timeout, 90)
        self.assertEqual(command[-4], str(self.root / 'plugin'))
        self.assertEqual(command[-3], '--replay')
        started(1000 + self.calls)
        Path(command[-1]).write_text(json.dumps(self.raw))
        Path(command[-1]).with_suffix('.json.columns').write_bytes(self.columns)
        log.write_text('NATIVE_SECONDS=0.010000000\n')

    def host(self, excluded=()):
        records = blocks.read(self.out / 'records.json')
        return {row['block_id']: {'status': 'excluded' if row['block_id'] in excluded else 'eligible'} for row in records['blocks']}

    def analyze(self, host=None):
        with patch.object(blocks, 'host_classification', return_value=self.host() if host is None else host):
            return blocks.analyze(self.manifest, self.out / 'records.json')

    def test_fixed_balanced_schedule_and_six_case_pilot_300_calls(self):
        self.spec['design']['attempts'] = 12
        item = self.spec['comparisons'][0]
        self.spec['comparisons'] = [{**copy.deepcopy(item), 'id': f'case{i}', 'workload': f'work{i}'} for i in range(6)]
        schedule = blocks.schedule(self.spec)
        self.assertEqual(schedule, blocks.schedule(copy.deepcopy(self.spec)))
        self.assertEqual(2 * len(schedule['warmups']) + 4 * len(schedule['blocks']), 300)
        orders = []
        for round_id in range(12):
            orders.append(tuple(row['comparison_id'] for row in schedule['blocks'] if row['round'] == round_id))
        self.assertGreater(len(set(orders)), 1)
        for item in self.spec['comparisons']:
            orientations = [row['orientation'] for row in schedule['blocks'] if row['comparison_id'] == item['id']]
            self.assertEqual(orientations.count('ABBA'), 6)
            self.assertEqual(orientations.count('BAAB'), 6)

    def test_schema_rejects_unfrozen_or_invalid_designs(self):
        for change in [{'attempts': 7}, {'analysis_unit': 'session'}, {'alpha': {'numerator': 1, 'denominator': 20}},
                       {'warmups': 0}, {'timeout_s': float('nan')}]:
            spec = copy.deepcopy(self.spec); spec['design'].update(change)
            with self.assertRaises(ValueError): blocks.validate_spec(spec)
        spec = copy.deepcopy(self.spec); spec['comparisons'][0]['candidate']['label'] = 'A'
        with self.assertRaises(ValueError): blocks.validate_spec(spec)

    def test_exact_adapter_checks_qualified_slots_plugin_and_model(self):
        for change in ['missing', 'duplicate', 'plugin', 'model']:
            spec = copy.deepcopy(self.spec)
            if change in ['missing', 'duplicate']:
                proof = blocks.read(self.root / 'qualification.json')
                proof['records'].pop() if change == 'missing' else proof['records'].append(proof['records'][0])
                path = self.root / (change + '.json'); path.write_text(json.dumps(proof))
                spec['comparisons'][0]['validation']['qualification'] = blocks.reference(path)
            elif change == 'plugin':
                spec['comparisons'][0]['candidate']['plugin'] = blocks.reference(self.root / 'harness')
            else:
                spec['comparisons'][0]['validation']['poisson'] = True
            with self.assertRaises(ValueError): blocks.ExactScience(spec['comparisons'][0])

    def test_complete_collection_exact_science_atomic_checkpoints_and_pid_events(self):
        self.freeze()
        snapshots = []
        original = blocks.atomic_json
        def save(path, value, **kwargs):
            if Path(path).name == 'records.json': snapshots.append(copy.deepcopy(value))
            return original(path, value, **kwargs)
        with patch.object(blocks, 'atomic_json', side_effect=save):
            result = blocks.collect(self.manifest, self.out, execute=self.execute)
        self.assertEqual(self.calls, 26)
        self.assertEqual(result['status'], 'complete')
        self.assertEqual(sum(row['status'] == 'complete' for row in result['blocks']), 6)
        self.assertTrue(all(any(sum(b['status'] == 'complete' for b in snap['blocks']) == n for snap in snapshots) for n in range(7)))
        events = [json.loads(line) for line in (self.out / 'host-events.jsonl').read_text().splitlines()]
        starts = [event for event in events if event['event'] == 'call_start']
        self.assertEqual(len(starts), 26)
        self.assertEqual(len({event['process_token'] for event in starts}), 26)
        self.assertTrue(all(event['native_pid'] > 1000 and 'seconds' not in event for event in events if event['event'] == 'call_start'))
        self.assertFalse(list(self.out.glob('*.columns')))
        report = self.analyze()
        self.assertTrue(report['conditional_statistical_pass'])
        self.assertFalse(report['qualification_pass'])
        self.assertTrue(report['summary'][0]['is_aa'])
        self.assertTrue(report['summary'][0]['dependence_diagnostics']['not_an_independence_certificate'])
        self.assertEqual(report['summary'][0]['order_diagnostics']['ABBA']['n'], 3)
        self.assertEqual(report['summary'][0]['all_complete_solve_times_descriptive']['candidate']['mean'], .01)
        with self.assertRaises(FileExistsError): blocks.collect(self.manifest, self.out, execute=self.execute)

    def test_timeout_consumes_slot_retains_partial_and_stops_without_replacement(self):
        self.freeze()
        def execute(*args):
            self.execute(*args)
            if self.calls == 4: raise subprocess.TimeoutExpired('synthetic', 1)
        with self.assertRaises(subprocess.TimeoutExpired): blocks.collect(self.manifest, self.out, execute=execute)
        records = blocks.read(self.out / 'records.json')
        self.assertEqual(self.calls, 4)
        self.assertEqual([row['status'] for row in records['blocks']], ['failed'] + ['not_run'] * 5)
        failed = records['blocks'][0]['calls'][-1]
        self.assertEqual(failed['failure_kind'], 'timeout')
        self.assertEqual(set(failed['artifacts']), {'result', 'columns', 'log'})
        report = self.analyze(self.host(excluded=[row['block_id'] for row in records['blocks']]))
        self.assertTrue(report['scientific_or_execution_failure'])
        self.assertFalse(report['conditional_statistical_pass'])

    def test_science_failure_cannot_be_noise_excluded(self):
        self.freeze()
        def execute(*args):
            self.execute(*args)
            if self.calls == 3:
                path = Path(args[0][-1]); raw = blocks.read(path)
                raw['matrices']['RB']['bits'][0] = bits(2.); path.write_text(json.dumps(raw))
        with self.assertRaisesRegex(ValueError, 'Exact science'): blocks.collect(self.manifest, self.out, execute=execute)
        report = self.analyze(self.host(excluded=['b000000']))
        self.assertFalse(report['conditional_statistical_pass'])
        self.assertTrue(report['scientific_or_execution_failure'])

    def test_missing_monitor_retains_all_descriptive_noise_without_claim(self):
        self.freeze(); blocks.collect(self.manifest, self.out, execute=self.execute)
        report = blocks.analyze(self.manifest, self.out / 'records.json')
        self.assertTrue(report['missing_monitor_evidence'])
        self.assertFalse(report['conditional_statistical_pass'])
        self.assertEqual(len(report['summary'][0]['all_complete_block_ratios_descriptive']), 6)
        report = self.analyze(self.host(excluded=['b000000']))
        self.assertEqual(report['summary'][0]['eligible_blocks'], 5)
        self.assertTrue(report['summary'][0]['excess_median_interval']['upper_unbounded'])
        self.assertFalse(report['conditional_statistical_pass'])

    def test_frozen_inputs_manifests_logs_and_call_assignment_are_checked(self):
        self.freeze(); blocks.collect(self.manifest, self.out, execute=self.execute)
        original = blocks.read(self.out / 'records.json')
        tampered = copy.deepcopy(original)
        tampered['blocks'][0]['calls'][0]['comparison_id'] = 'different'
        blocks.atomic_json(self.out / 'records.json', tampered)
        with self.assertRaisesRegex(ValueError, 'Misassigned'): self.analyze()
        blocks.atomic_json(self.out / 'records.json', original)
        (self.out / 'c0000000.log').write_text('NATIVE_SECONDS=0.02\n')
        with self.assertRaisesRegex(ValueError, 'log changed'): self.analyze()
        (self.root / 'plugin').write_text('changed')
        with self.assertRaisesRegex(ValueError, 'binding changed'): blocks.load_manifest(self.manifest)

    def test_fresh_seal_and_strict_json(self):
        self.freeze()
        with self.assertRaisesRegex(ValueError, 'overwrite'): self.freeze()
        envelope = blocks.read(self.manifest); envelope['value']['schedule']['blocks'].reverse()
        self.manifest.write_text(json.dumps(envelope))
        with self.assertRaisesRegex(ValueError, 'seal changed'): blocks.load_manifest(self.manifest)
        self.manifest.write_text('{"a":1,"a":2}')
        with self.assertRaisesRegex(ValueError, 'Duplicate'): blocks.read(self.manifest)

    def test_interrupted_atomic_write_preserves_checkpoint_and_allows_failure_save(self):
        path = self.root / 'checkpoint.json'
        blocks.atomic_json(path, {'status': 'running'})
        with patch.object(blocks.os, 'replace', side_effect=KeyboardInterrupt()):
            with self.assertRaises(KeyboardInterrupt): blocks.atomic_json(path, {'status': 'complete'})
        self.assertEqual(blocks.read(path), {'status': 'running'})
        self.assertFalse(path.with_name(path.name + '.pending').exists())
        blocks.atomic_json(path, {'status': 'failed'})
        self.assertEqual(blocks.read(path), {'status': 'failed'})

    def test_budget_exhaustion_records_failure_without_dispatch(self):
        self.freeze()
        with patch.object(blocks.time, 'monotonic', side_effect=[0., 0., 901., 901., 901.]):
            with self.assertRaises(TimeoutError): blocks.collect(self.manifest, self.out, execute=self.execute)
        records = blocks.read(self.out / 'records.json')
        self.assertEqual(self.calls, 0)
        self.assertEqual(records['warmups'][0]['calls'][0]['failure_kind'], 'timeout')
        self.assertTrue(all(row['status'] == 'not_run' for row in records['blocks']))

    def test_monitor_binding_complete_coverage_and_allowed_reasons(self):
        self.freeze(); blocks.collect(self.manifest, self.out, execute=self.execute)
        context = blocks.monitor_context(self.manifest, self.out / 'records.json')
        bound = blocks.reference(self.root / 'declaration')
        self.spec['monitor'] = {'validator': bound, 'policy': bound, 'dependencies': []}
        receipt = {'status': 'complete', 'policy_sha256': bound['sha256'],
                   **{key + '_sha256': context[key]['sha256'] for key in ['manifest', 'records', 'events']},
                   'blocks': [{'block_id': row['block_id'], 'status': 'eligible', 'reason': None, 'evidence': [bound]}
                              for row in context['schedule']]}
        with patch.object(blocks, 'module_at', return_value=types.SimpleNamespace(validate_receipt=lambda *a: receipt)):
            self.assertEqual(len(blocks.host_classification(self.spec, context, 'unused')), 6)
            receipt['blocks'][0].update(status='excluded', reason='known_competing_compute')
            self.assertEqual(blocks.host_classification(self.spec, context, 'unused')['b000000']['status'], 'excluded')
            receipt['blocks'][0]['reason'] = 'ordinary_os_noise'
            with self.assertRaisesRegex(ValueError, 'not an exclusion'): blocks.host_classification(self.spec, context, 'unused')
            receipt['blocks'].pop(0)
            with self.assertRaisesRegex(ValueError, 'omitted'): blocks.host_classification(self.spec, context, 'unused')


class RoutingAndProcess(unittest.TestCase):
    def test_routing_checks_qualified_fingerprint_and_independent_oracle(self):
        adapter = blocks.RoutingScience.__new__(blocks.RoutingScience)
        adapter.meta, adapter.reader, adapter.expected = {}, {'candidate': 'auto'}, {'candidate': 'expected'}
        adapter.cmg = types.SimpleNamespace(read_run=Mock(return_value=({'raw': 1}, {'science_sha256': 'expected'})))
        adapter.runner = types.SimpleNamespace(check_oracle=Mock())
        self.assertTrue(adapter.check('candidate', Path('result.json'), Path('log'))['science_pass'])
        adapter.runner.check_oracle.assert_called_once_with({'raw': 1}, Path('result.json.columns'), {})
        adapter.cmg.read_run.return_value = ({}, {'science_sha256': 'different'})
        with self.assertRaisesRegex(ValueError, 'qualified science'): adapter.check('candidate', Path('result.json'), Path('log'))
        adapter.cmg.read_run.return_value = ({}, {'science_sha256': 'expected'})
        adapter.runner.check_oracle.side_effect = ValueError('independent truth mismatch')
        with self.assertRaisesRegex(ValueError, 'independent truth'): adapter.check('candidate', Path('result.json'), Path('log'))

    def test_cancellation_preserves_original_failure_when_cleanup_fails(self):
        process = Mock(pid=10)
        process.wait.side_effect = KeyboardInterrupt('stop')
        process.poll.return_value = None
        process.kill.side_effect = PermissionError('cleanup')
        with tempfile.TemporaryDirectory() as directory, patch.object(blocks.subprocess, 'Popen', return_value=process):
            with self.assertRaises(KeyboardInterrupt) as captured:
                blocks.run_native(['fake'], Path(directory) / 'log', 1, lambda pid: None)
            self.assertIn('cleanup could not be confirmed', captured.exception.__notes__[0])
            self.assertNotIn('start_new_session', blocks.subprocess.Popen.call_args.kwargs)


if __name__ == '__main__': unittest.main()
