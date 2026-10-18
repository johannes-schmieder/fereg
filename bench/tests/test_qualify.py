"""Regression guards for the single-round runner; no native benchmark campaign."""
import copy
import json
import os
from pathlib import Path
import sys
import signal
import tempfile
import threading
import time
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

    def test_execution_modules_are_bound_verified_once_and_required_by_report(self):
        manifest = self.plan()
        self.assertEqual(set(manifest['execution_sources']), {'qualify', *qualify.SOURCE_MODULES})
        hashed, actual = [], qualify.digest
        def count(path):
            hashed.append(Path(path))
            return actual(path)
        with patch.object(qualify, 'digest', side_effect=count):
            record = qualify.run(self.manifest, self.output, execute=self.execute)
        for path in qualify.execution_sources().values():
            self.assertEqual(hashed.count(path), 1)
        self.assertEqual(record['execution_sources'], manifest['execution_sources'])
        record['execution_sources'].pop('routing_science')
        qualify.write(self.output / 'run-0-0/results.json', record)
        self.assertEqual(qualify.report(self.manifest, self.output)['status'], 'needs_discussion')

    def test_source_mismatch_and_drift_block_without_mutating_real_modules(self):
        source = self.root / 'checker.py'
        source.write_text('original')
        with patch.object(qualify, 'execution_sources', return_value={'checker': source}):
            self.plan()
            source.write_text('changed!')
            record = qualify.run(self.manifest, self.output, execute=self.execute)
            self.assertIn('source identity', record['failures'][-1]['error'])
            self.assertFalse(self.calls)
        # A separate output permits a fresh synthetic job, not a campaign retry.
        self.manifest.unlink()
        self.output = self.root / 'target/tmp/other'
        with patch.object(qualify, 'execution_sources', return_value={'checker': source}):
            self.plan()
            def mutate(*args):
                result = self.execute(*args)
                source.write_text('drift!!!')
                return result
            record = qualify.run(self.manifest, self.output, execute=mutate)
            self.assertEqual(record['status'], 'blocked')
            self.assertEqual(len(self.calls), 1)

    def test_deadline_interrupts_metadata_and_is_not_resumable(self):
        self.config['budget_s'] = 1
        self.plan()
        with patch.object(qualify, 'metadata', side_effect=lambda *_: time.sleep(2)):
            record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['failures'][-1]['kind'], 'budget')
        self.assertFalse(record['budget_compliant'])
        self.assertFalse(self.calls)
        with self.assertRaisesRegex(ValueError, 'operational blocker'):
            qualify.run(self.manifest, self.output, resume=True, execute=self.execute)

    def test_delayed_final_write_cannot_publish_success(self):
        self.config['budget_s'] = 1
        self.plan()
        original = qualify.write
        def slow_write(path, value):
            original(path, value)
            if value.get('status') == 'complete' and Path(path).name == 'results.json':
                time.sleep(2)
        with patch.object(qualify, 'write', side_effect=slow_write):
            record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['failures'][-1]['kind'], 'budget')
        self.assertEqual(qualify.read(self.output / 'run-0-0/results.json')['status'], 'blocked')

    def test_science_stages_cannot_overrun_even_if_alarm_delivery_is_delayed(self):
        self.config['budget_s'] = 1
        self.plan()
        def delayed(*_):
            # Models a C-extension operation which returns after the deadline.
            signal.signal(signal.SIGALRM, signal.SIG_IGN)
            time.sleep(1.05)
        self.oracle.side_effect = delayed
        record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['failures'][-1]['kind'], 'budget')
        self.assertEqual(len(self.calls), 1)

    def test_report_rejects_over_budget_or_unbound_checking_sources(self):
        record = self.run_round()
        path = self.output / 'run-0-0/results.json'
        record['elapsed_s'] = self.config['budget_s'] + .01
        qualify.write(path, record)
        self.assertIn('unverified run budget', str(qualify.report(self.manifest, self.output)['errors']))
        manifest = qualify.read(self.manifest)
        manifest['execution_sources']['routing_oracle'] = '0' * 64
        qualify.write(self.manifest, manifest)
        self.assertIn('source identity', str(qualify.report(self.manifest, self.output)['errors']))

    def test_observer_shutdown_overrun_fails_after_cleanup(self):
        self.config['budget_s'] = 1
        self.plan()
        stopped = []
        class SlowMonitor(Monitor):
            def __exit__(self, *_):
                time.sleep(1.05)
                stopped.append(True)
        with patch('qualify_scc.MacMonitor', SlowMonitor):
            record = qualify.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(stopped, [True])
        self.assertEqual(record['failures'][-1]['kind'], 'budget')
        self.assertGreater(record['cleanup_overrun_s'], 0)

    def test_non_main_thread_rejected_before_output_mutation(self):
        self.plan()
        errors = []
        def invoke():
            try:
                qualify.run(self.manifest, self.output, execute=self.execute)
            except ValueError as error:
                errors.append(str(error))
        thread = threading.Thread(target=invoke)
        thread.start()
        thread.join(2)
        self.assertEqual(errors, ['run requires the main thread'])
        self.assertFalse(self.output.exists())

    def test_alarm_kills_owned_process_group_reaps_child_and_restores_handlers(self):
        with tempfile.TemporaryDirectory() as tmp:
            log, marker = Path(tmp) / 'child.log', Path(tmp) / 'survivor'
            handlers = {sig: signal.getsignal(sig) for sig in (signal.SIGALRM, signal.SIGINT, signal.SIGTERM)}
            code = ('import os,time,pathlib; print(os.getpid(),flush=True); child=os.fork(); '
                    'time.sleep(.6 if child==0 else 20); '
                    f'pathlib.Path({str(marker)!r}).write_text("survived")')
            with patch.object(os, 'killpg', wraps=os.killpg) as kill:
                with self.assertRaises(qualify.BudgetExpired):
                    with qualify.Deadline(.2, time.monotonic()):
                        qualify.run_process([sys.executable, '-c', code], log, 5)
                kill.assert_called_once_with(int(log.read_text().strip()), signal.SIGKILL)
            with self.assertRaises(ChildProcessError):
                os.waitpid(int(log.read_text().strip()), os.WNOHANG)
            time.sleep(.65)
            self.assertFalse(marker.exists(), 'owned descendant survived deadline cleanup')
            self.assertEqual(signal.getitimer(signal.ITIMER_REAL), (0., 0.))
            self.assertEqual(handlers, {sig: signal.getsignal(sig) for sig in handlers})

    def test_alarm_during_native_timeout_cleanup_waits_for_reap(self):
        log = self.root / 'timeout.log'
        original_wait, reaped = os.wait4, []
        def wait(pid, options):
            if options == 0:
                os.kill(os.getpid(), signal.SIGALRM)
                value = original_wait(pid, options)
                reaped.append(pid)
                return value
            return original_wait(pid, options)
        with patch.object(os, 'wait4', side_effect=wait):
            with self.assertRaises(TimeoutError) as failure:
                with qualify.Deadline(2, time.monotonic()):
                    qualify.run_process([sys.executable, '-c', 'import time; time.sleep(5)'], log, .05)
        self.assertIs(type(failure.exception), TimeoutError)
        self.assertEqual(len(reaped), 1)
        with self.assertRaises(ChildProcessError):
            os.waitpid(reaped[0], os.WNOHANG)

    def test_alarm_delivered_to_observer_during_cleanup_cannot_interrupt_reap(self):
        stop = threading.Event()
        observer = threading.Thread(target=stop.wait)
        observer.start()
        kill, wait = os.killpg, os.wait4
        owned, reaped, failures = [], [], []
        def during_kill(pid, sig):
            owned.append(pid)
            signal.pthread_kill(observer.ident, signal.SIGALRM)
            time.sleep(.01)  # Dispatch Python's pending handler before wait4.
            kill(pid, sig)
        def during_wait(pid, options):
            value = wait(pid, options)
            if options == 0:
                reaped.append(pid)
            return value
        try:
            with patch.object(os, 'killpg', side_effect=during_kill), patch.object(os, 'wait4', side_effect=during_wait):
                with qualify.Deadline(2, time.monotonic()) as deadline:
                    try:
                        qualify.run_process([sys.executable, '-c', 'import time; time.sleep(5)'],
                                            self.root / 'observer-timeout.log', .05)
                    except TimeoutError as error:
                        failures.append(error)
                    self.assertTrue(all(signal.getsignal(sig) == deadline.interrupt for sig in deadline.handlers))
            self.assertEqual(reaped, owned)
            self.assertEqual(len(failures), 1)
            self.assertIs(type(failures[0]), TimeoutError)
        finally:
            stop.set()
            observer.join(1)
            # Clean even the deliberately reproduced pre-fix leak.
            for pid in owned:
                if pid not in reaped:
                    try:
                        kill(pid, signal.SIGKILL)
                        wait(pid, 0)
                    except (ProcessLookupError, ChildProcessError):
                        pass

    def test_manifest_alias_retarget_blocks_even_when_original_target_is_unchanged(self):
        self.plan()
        alternate = self.root / 'alternate'
        alternate.write_bytes(self.plugin.read_bytes())
        alias = self.root / 'alias'
        alias.symlink_to(self.plugin)
        manifest = qualify.read(self.manifest)
        manifest['files']['alias'] = manifest['files'].pop('plugin')
        for artifact in manifest['artifacts'].values():
            artifact['path'] = 'alias'
        qualify.write(self.manifest, manifest)
        def retarget(*args):
            value = self.execute(*args)
            alias.unlink()
            alias.symlink_to(alternate)
            return value
        record = qualify.run(self.manifest, self.output, execute=retarget)
        self.assertEqual(record['status'], 'blocked')
        self.assertIn('alias changed', record['failures'][-1]['error'])
        self.assertEqual(len(self.calls), 1)

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
