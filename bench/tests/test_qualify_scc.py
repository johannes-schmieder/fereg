import json
import os
import subprocess
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify_scc as scc


def allocation():
    host = {'hostname': 'scc-a.scc.bu.edu', 'job_id': '123', 'task_id': '2',
            'slots': 28, 'affinity': list(range(28)), 'queue': 'batch@scc-a.scc.bu.edu'}
    raw = {'pe_hostfile': 'scc-a 28 batch@scc-a.scc.bu.edu\n',
           'pe': 'pe_name omp28\nallocation_rule $pe_slots\n',
           'quota': 'name max_slots_per_host\nenabled TRUE\nlimit hosts {*} to slots=$num_proc\n',
           'complexes': 'num_proc np INT == YES NO 0 0\nslots s INT <= YES YES 1 0\n',
           'host': 'hostname scc-a\nprocessors 28\ncomplex_values num_proc=28\nload_values num_proc=28\n',
           'topology': '\n'.join(f'{i},{i},0,Y' for i in range(28))}
    return host, raw


def census(task='2', slots='28', extra=''):
    job = 'job_number: 123\nproject: welfgr\nparallel environment: omp28 range: 28\nhard resource_list: num_proc=28,h_rt=12:00:00\n'
    xml = ('<job_info><queue_info><Queue-List><name>batch@scc-a.scc.bu.edu</name>'
           f'<job_list><JB_job_number>123</JB_job_number><slots>{slots}</slots><tasks>{task}</tasks></job_list>'
           f'{extra}</Queue-List></queue_info></job_info>')
    return job, xml


class ReservationTests(unittest.TestCase):
    def test_full_reservation_and_array_identity(self):
        host, raw = allocation()
        self.assertEqual(scc.validate_reservation(raw, host, 28), host['queue'])
        scc.validate_census(*census(), host)
        scc.validate_census(*census(), {**host, 'queue': 'batch@scc-a'})

    def test_slots_alone_or_missing_all_queue_quota_cannot_pass(self):
        host, raw = allocation()
        for key, value in [('quota', raw['quota'].replace('TRUE', 'FALSE')),
                           ('quota', raw['quota'].replace('{*}', '{batch}')),
                           ('pe', raw['pe'].replace('$pe_slots', '1')),
                           ('complexes', raw['complexes'].replace('YES YES', 'YES NO')),
                           ('host', raw['host'].replace('num_proc=28', 'num_proc=56'))]:
            with self.subTest(key=key, value=value), self.assertRaises(scc.OperationalError):
                scc.validate_reservation({**raw, key: value}, host, 2)

    def test_partial_affinity_smt_and_worker_overcommit_block(self):
        host, raw = allocation()
        for changed_host, changed_raw, workers in [({**host, 'affinity': list(range(27))}, raw, 2),
                (host, {**raw, 'topology': '\n'.join(f'{i},{i//2},0,Y' for i in range(28))}, 2),
                (host, raw, 29)]:
            with self.assertRaises(scc.OperationalError):
                scc.validate_reservation(changed_raw, changed_host, workers)

    def test_co_tenant_other_task_missing_task_wrong_slots_block(self):
        host, _ = allocation()
        for args in [{'task': '3'}, {'task': ''}, {'slots': '14'},
                     {'extra': '<job_list><JB_job_number>456</JB_job_number></job_list>'}]:
            with self.subTest(args=args), self.assertRaises(scc.OperationalError):
                scc.validate_census(*census(**args), host)

    def test_duplicate_configuration_is_not_silently_overwritten(self):
        with self.assertRaises(scc.OperationalError):
            scc.fields('processors 28\nprocessors 56\n')


class LifecycleTests(unittest.TestCase):
    def monitor(self, directory):
        monitor = scc.HostMonitor(directory, 2, process_interval=.01, census_interval=.01)
        def sample(kind):
            monitor.samples[kind] = {'start': scc.time.monotonic_ns(), 'finish': scc.time.monotonic_ns(), 'count': 1}
        monitor._sample = sample
        monitor._reservation = lambda: None
        return monitor

    def environment(self):
        return patch.dict(os.environ, {**scc.LIBRARY_LIMITS, 'JOB_ID': '123', 'SGE_TASK_ID': '2',
                                       'NSLOTS': '28', 'PE': 'omp28'})

    def test_normal_and_exception_exit_join_helpers_and_write_report(self):
        for fail in [False, True]:
            with tempfile.TemporaryDirectory() as tmp, self.environment(), patch.object(scc.sys, 'platform', 'linux'), \
                    patch.object(scc.os, 'sched_getaffinity', return_value=set(range(28)), create=True):
                monitor = self.monitor(tmp)
                try:
                    with monitor:
                        monitor.check()
                        if fail:
                            raise LookupError('native failure')
                except LookupError:
                    self.assertTrue(fail)
                self.assertTrue(all(not t.is_alive() for t in monitor.threads))
                self.assertTrue(Path(tmp, 'host.json').exists())
                self.assertTrue(monitor.report()['isolation_ok'])

    def test_observer_fault_blocks_and_is_retained(self):
        with tempfile.TemporaryDirectory() as tmp:
            monitor = self.monitor(tmp)
            monitor.started = scc.time.monotonic_ns()
            for kind in monitor.intervals:
                monitor._sample(kind)
            monitor._error(scc.OperationalError('scheduler unavailable'))
            with self.assertRaisesRegex(scc.OperationalError, 'scheduler unavailable'):
                monitor.check()
            self.assertFalse(monitor.report()['isolation_ok'])

    def test_background_fault_stops_loop_and_shutdown_reaps(self):
        with tempfile.TemporaryDirectory() as tmp:
            monitor = self.monitor(tmp)
            seen = threading.Event()
            def failed(kind):
                seen.set()
                raise scc.OperationalError('missing census')
            monitor._sample = failed
            thread = threading.Thread(target=monitor._loop, args=('census',))
            monitor.threads.append(thread)
            thread.start()
            self.assertTrue(seen.wait(1))
            monitor._stop()
            self.assertFalse(thread.is_alive())
            self.assertIn('missing census', monitor.errors)

    def test_process_monitor_excludes_owned_children_and_retains_ordinary_noise(self):
        def process(pid, parent, executable):
            return {'pid': pid, 'ppid': parent, 'start': 'Mon Oct 19 00:00:00 2026',
                    'state': 'R', 'cpu_seconds': 1, 'cpu_percent': 50, 'rss_kib': 100,
                    'executable': executable, 'executable_raw': executable, 'command': executable}
        root = process(os.getpid(), 1, 'python3')
        for other, passes in [(process(987654, os.getpid(), 'gcc'), True),
                              (process(987654, 1, 'desktop-service'), True),
                              (process(987654, 1, 'gcc'), False)]:
            with tempfile.TemporaryDirectory() as tmp:
                monitor = scc.HostMonitor(tmp, 2)
                sample = {'monotonic_ns': scc.time.monotonic_ns(), 'finished_monotonic_ns': scc.time.monotonic_ns(),
                          'processes': [root, other]}
                with patch.object(scc.block_host, 'snapshot', return_value=sample):
                    if passes:
                        monitor._sample('process')
                    else:
                        with self.assertRaisesRegex(scc.OperationalError, 'competing compute'):
                            monitor._sample('process')
                self.assertTrue(Path(tmp, 'host-observations.jsonl').exists())

    def test_early_platform_error_is_not_masked_by_output_error(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(scc.sys, 'platform', 'darwin'):
            monitor = scc.HostMonitor(Path(tmp) / 'missing', 2)
            with self.assertRaisesRegex(scc.OperationalError, 'Linux SGE'):
                monitor.__enter__()

    def test_mac_uses_only_process_observation_without_scheduler_or_accounting(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(scc.sys, 'platform', 'darwin'):
            monitor = scc.MacMonitor(tmp, 2)
            order = []
            def sample(kind):
                order.append(kind)
                monitor.samples[kind] = {'start': scc.time.monotonic_ns(), 'finish': scc.time.monotonic_ns(), 'count': len(order)}
            monitor._sample = sample
            with patch.object(monitor, 'execute') as execute:
                with monitor:
                    monitor.check()
                execute.assert_not_called()
            self.assertEqual(order, ['process', 'process'])
            self.assertTrue(monitor.report()['isolation_ok'])
            self.assertFalse(monitor.report()['accounting_required'])
            self.assertTrue(all(not thread.is_alive() for thread in monitor.threads))

    def test_exit_samples_process_tail_before_stopping_helpers(self):
        with tempfile.TemporaryDirectory() as tmp, self.environment(), patch.object(scc.sys, 'platform', 'linux'), \
                patch.object(scc.os, 'sched_getaffinity', return_value=set(range(28)), create=True):
            monitor = self.monitor(tmp)
            with monitor:
                original = monitor._sample
                def sample(kind):
                    if kind == 'process':
                        self.assertFalse(monitor.stop.is_set())
                    else:
                        self.assertTrue(monitor.stop.is_set())
                    original(kind)
                monitor._sample = sample
            self.assertTrue(monitor.report()['isolation_ok'])

    def test_command_timeout_kills_and_reaps_own_process_group(self):
        with patch.object(scc.subprocess, 'Popen') as spawn, patch.object(scc.os, 'killpg') as kill:
            child = spawn.return_value.__enter__.return_value
            child.pid = 567
            child.communicate.side_effect = [scc.subprocess.TimeoutExpired('qstat', 10), ('', '')]
            with self.assertRaises(scc.subprocess.TimeoutExpired):
                scc.command(['qstat'])
            kill.assert_called_once_with(567, scc.signal.SIGKILL)
            self.assertEqual(child.communicate.call_count, 2)


class WrapperTests(unittest.TestCase):
    def test_array_mapping_uses_common_output_root_and_rejects_outside_task(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest, capture, output = (root / name for name in ('manifest.json', 'capture.json', 'output'))
            manifest.write_text(json.dumps({'shards': 3, 'replicas': 2}))
            python = root / 'python3'
            python.write_text(f'#!{sys.executable}\nimport json,os,pathlib,sys\n'
                              f'if sys.argv[1] == "-": os.execv({sys.executable!r}, [{sys.executable!r}, *sys.argv[1:]])\n'
                              'pathlib.Path(os.environ["CAPTURE"]).write_text(json.dumps(sys.argv[1:]))\n')
            python.chmod(0o755)
            wrapper = Path(scc.__file__).with_name('qualify.sge')
            env = {**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH'],
                   'CAPTURE': str(capture), 'SGE_TASK_ID': '5'}
            argv = ['bash', '-c', 'module() { :; }; export -f module; exec bash "$@"',
                    'wrapper-test', str(wrapper), str(manifest), str(output)]
            result = subprocess.run(argv, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(capture.read_text()),
                             ['bench/qualify.py', 'run', str(manifest), '--output', str(output), '--shard', '1', '--replica', '1'])
            capture.unlink()
            result = subprocess.run(argv, env={**env, 'SGE_TASK_ID': '7'}, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(capture.exists())


if __name__ == '__main__':
    unittest.main()
