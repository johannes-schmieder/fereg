"""Regression guards for the single-round runner; no native benchmark campaign."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify
from test_cmgschur import result, case_bytes, bits


class Monitor:
    def __init__(self, directory, *_): self.directory = directory
    def __enter__(self):
        self.directory.mkdir(parents=True, exist_ok=True)
        with (self.directory / 'host-observations.jsonl').open('x'): pass
        return self
    def __exit__(self, *_): pass
    def check(self): pass
    def report(self): return {'isolation_ok': True}


class Qualification(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.output = self.root / 'target/tmp/round'
        self.source = self.root / 'input.case'
        self.source.write_bytes(case_bytes())
        self.plugin = self.root / 'plugin'
        self.plugin.write_bytes(b'actual artifact')
        (self.root / 'harness').write_bytes(b'actual harness')
        artifact = {'path': 'plugin', 'revision': 'abc', 'build': 'test fixture', 'request': 'auto', 'families': []}
        self.config = {'scope': 'exact', 'platform': 'Darwin', 'artifacts': {r: dict(artifact) for r in
                       ('original', 'incumbent', 'candidate')}, 'harness': 'harness',
                       'cases': [{'id': 'fixture', 'family': 'no-fe', 'input': 'input.case',
                                  'oracle': {'kind': 'dense'}, 'memory_limit_bytes': None}],
                       'threads': [1, 4], 'blocks': 2, 'shards': 1, 'replicas': 1,
                       'seed': 7, 'timeout_s': 5, 'budget_s': 30, 'coverage': 'smoke'}
        self.meta = {'rows': 2, 'columns': 6, 'outputs': dict(zip(
            ('sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed'), range(6))),
            'categorical_intercepts': True, 'absorbed_terms': 0, 'saved': []}
        self.data = np.array([[1., 1.], [.2, -.2], [0., 0.], [.3, .4], [.5, .2], [.2, -.2]])
        self.calls = []
        self.addCleanup(patch.stopall)
        patch.object(qualify, 'ROOT', self.root).start()
        patch.object(qualify.platform, 'system', return_value='Darwin').start()
        patch.object(qualify, 'metadata', return_value=self.meta).start()
        patch.object(qualify.routing_cases, 'inspect_case', return_value=self.meta).start()
        self.oracle = patch.object(qualify.science, 'check_oracle').start()
        patch('qualify_scc.MacMonitor', Monitor).start()

    def plan(self):
        config = self.root / 'config.json'
        qualify.write(config, self.config)
        self.manifest = self.root / 'plan.json'
        return qualify.plan(config, self.manifest)

    def execute(self, command, log, timeout):
        self.calls.append(command)
        target = Path(command[-1])
        raw = result()
        qualify.write(target, {**raw, 'columns': []})
        self.data.tofile(target.with_suffix('.json.columns'))
        log.write_text('NATIVE_SECONDS=0.010000\n')
        return {'maxrss_bytes': 1024}

    def run_round(self, execute=None):
        self.plan()
        return qualify.run(self.manifest, self.output, execute=execute or self.execute)

    def test_complete_round_deduplicates_artifacts_and_checks_oracle_once_per_worker(self):
        record = self.run_round()
        self.assertEqual(record['status'], 'complete', record)
        self.assertEqual(len(self.calls), 10)  # Two worker cells, 1 warmup + 4 calls.
        self.assertEqual(self.oracle.call_count, 2)
        for cell in record['cases']:
            self.assertEqual(len(set(cell['aliases'].values())), 1)
            self.assertEqual(len(cell['blocks']), 2)
        self.assertFalse(list(self.output.rglob('*.columns')))
        self.assertFalse(list(self.output.rglob('*.case')))
        self.assertEqual(qualify.report(self.manifest, self.output)['status'], 'smoke_complete')

    def test_artifact_is_verified_once_per_job_not_per_call(self):
        self.plan()
        actual = qualify.digest
        hashed = []
        def count(path):
            hashed.append(Path(path))
            return actual(path)
        with patch.object(qualify, 'digest', side_effect=count):
            self.assertEqual(qualify.run(self.manifest, self.output, execute=self.execute)['status'], 'complete')
        self.assertEqual(hashed.count(self.plugin), 1)
        self.assertFalse(any(p.suffix == '.case' and p != self.source for p in hashed))

    def test_rejects_changed_artifact_before_native_calls(self):
        self.plan()
        self.plugin.write_bytes(b'changed')
        record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(record['failures'][-1]['kind'], 'operational')
        self.assertEqual(self.calls, [])

    def test_mutation_during_job_blocks_without_more_calls(self):
        def execute(*args):
            value = self.execute(*args)
            self.plugin.write_bytes(b'changed')
            return value
        record = self.run_round(execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(len(self.calls), 1)
        self.assertIn('changed', record['failures'][-1]['error'])

    def test_scientific_failure_never_moves_on_or_retries(self):
        self.oracle.side_effect = ValueError('independent covariance failed')
        record = self.run_round()
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(record['failures'][-1]['kind'], 'scientific')
        self.assertEqual(len(self.calls), 1)
        self.assertIn('discuss', record['next_action'])
        with self.assertRaisesRegex(ValueError, 'operational blocker'):
            qualify.run(self.manifest, self.output, resume=True, execute=self.execute)
        self.assertEqual(qualify.report(self.manifest, self.output)['status'], 'needs_discussion')

    def test_timed_output_change_fails_fixed_worker_repeat(self):
        def execute(*args):
            value = self.execute(*args)
            if len(self.calls) == 2:
                target = Path(args[0][-1])
                data = self.data.copy()
                data[1, 0] += 1e-12
                data.tofile(target.with_suffix('.json.columns'))
            return value
        record = self.run_round(execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertIn('fixed-worker repeat', record['failures'][-1]['error'])
        self.assertEqual(len(self.calls), 2)

    def test_native_timeout_is_not_operational_resume_permission(self):
        def timeout(*_): raise TimeoutError('solver timeout')
        record = self.run_round(timeout)
        self.assertEqual(record['failures'][-1]['kind'], 'native')
        with self.assertRaises(ValueError):
            qualify.run(self.manifest, self.output, resume=True, execute=self.execute)

    def test_oracle_is_required_and_public_activation_needs_hidden_equivalence(self):
        self.config['cases'][0]['oracle'] = {'kind': 'none'}
        with self.assertRaisesRegex(ValueError, 'independent'):
            self.plan()
        self.config['cases'][0]['oracle'] = {'kind': 'dense'}
        self.config['scope'] = 'many'
        self.config['artifacts']['candidate']['families'] = ['many']
        with self.assertRaisesRegex(ValueError, 'selected hidden'):
            self.plan()

    def test_report_rejects_missing_or_corrupted_results(self):
        self.run_round()
        file = self.output / 'run-0-0/results.json'
        record = qualify.read(file)
        for mutate in (lambda r: r['cases'].pop(),
                       lambda r: r['cases'][0]['blocks'][0]['calls'].pop(),
                       lambda r: r['cases'][0]['blocks'][0]['calls'][0].update(fingerprint='wrong'),
                       lambda r: r['cases'][0]['aliases'].update(original='original'),
                       lambda r: r['cases'][0].update(references={}, worker_signatures={}),
                       lambda r: r['cases'][0].update(host_attempt=15)):
            altered = copy.deepcopy(record)
            mutate(altered)
            qualify.write(file, altered)
            self.assertEqual(qualify.report(self.manifest, self.output)['status'], 'needs_discussion')
        file.unlink()
        self.assertEqual(qualify.report(self.manifest, self.output)['status'], 'needs_discussion')

    def test_operational_resume_reuses_complete_cells_with_their_host_evidence(self):
        self.config['cases'].append({**self.config['cases'][0], 'id': 'second'})
        self.plan()
        with patch.object(qualify, 'metadata', side_effect=[self.meta, OSError('input unavailable')]):
            record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertEqual(len(self.calls), 10)
        record = qualify.run(self.manifest, self.output, resume=True, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record)
        self.assertEqual(len(self.calls), 20)
        self.assertEqual(len(record['previous_hosts']), 1)
        self.assertEqual(qualify.report(self.manifest, self.output)['status'], 'smoke_complete')

    def test_public_preserve_cannot_bypass_selected_hidden_requirement(self):
        self.config['scope'] = 'many'
        self.config['artifacts']['candidate'].update(request='preserve', families=['many'])
        with self.assertRaisesRegex(ValueError, 'explicit auto'):
            self.plan()

    def test_environment_clears_instrumentation_and_limits_external_threads(self):
        self.run_round()
        command = self.calls[0]
        for key in qualify.ENVIRONMENT:
            index = command.index(key)
            self.assertEqual(command[index - 1], '-u')
        for key in qualify.THREAD_ENV:
            self.assertIn(key + '=1', command)
        self.assertIn('FEREG_SOLVER_TRACE=1', command)
        self.assertNotIn('FEREG_SOLVER_TRACE=1', self.calls[1])

    def test_outside_changed_family_and_hidden_output_must_be_exact(self):
        manifest = {'scope': 'many', 'artifacts': {'candidate': {'families': ['many']}}}
        first = ({}, None, {'fingerprint': 'one'})
        second = ({}, None, {'fingerprint': 'two'})
        with self.assertRaisesRegex(ValueError, 'unaffected'):
            qualify.compare_artifacts(manifest, self.meta, {'candidate': second, 'incumbent': first})
        with self.assertRaisesRegex(ValueError, 'hidden'):
            qualify.compare_artifacts(manifest, {**self.meta, 'absorbed_terms': 4},
                                      {'candidate': second, 'hidden': first})


class Statistics(unittest.TestCase):
    def test_mirrored_blocks_are_balanced_and_reproducible(self):
        blocks = qualify.orders(['a', 'b', 'c'], 6, 7)
        self.assertEqual(blocks, qualify.orders(['a', 'b', 'c'], 6, 7))
        for row in blocks:
            self.assertEqual(row[:3], list(reversed(row[3:])))
            self.assertEqual(sorted(row), ['a', 'a', 'b', 'b', 'c', 'c'])
        for i in range(0, 6, 2):
            self.assertEqual(blocks[i][:3], list(reversed(blocks[i+1][:3])))

    def test_uncertainty_does_not_invent_precision_and_allowance_is_per_cell(self):
        self.assertIsNone(qualify.interval([-1] * 5)['upper'])
        self.assertEqual(qualify.interval([-1] * 6)['upper'], -1)
        for baseline in (.001, 1.):
            allowance = max(.05 * baseline, .001)
            calls = [{'execution': a, 'seconds': s} for a, s in
                     [('a', baseline), ('b', baseline+allowance+.001)] for _ in range(2)]
            self.assertAlmostEqual(qualify.score(calls, 'a', 'b')['excess_s'], .001)

    def test_accounting_must_be_successful_and_match_actual_host_and_task(self):
        host = {'job_id': '123', 'task_id': '2', 'hostname': 'scc-node'}
        text = 'jobnumber 123\ntaskid 2\nhostname scc-node.bu.edu\nproject welfgr\nfailed 0\nexit_status 0\nend_time done\n'
        qualify.accounting(text, host)
        for bad in (text.replace('exit_status 0', 'exit_status 1'), text.replace('taskid 2', 'taskid 1'),
                    text.replace('scc-node.bu.edu', 'other'), text + 'jobnumber 123\n'):
            with self.assertRaises(ValueError):
                qualify.accounting(bad, host)


if __name__ == '__main__':
    unittest.main()
