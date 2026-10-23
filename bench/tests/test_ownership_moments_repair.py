"""Synthetic failed-cohort continuation tests; never launch a native process."""
import copy
from pathlib import Path
import unittest
from unittest.mock import patch

import ownership_cells as oc
import ownership_moments as moments
import ownership_moments_repair as repair
import qualify as q
import routing
import test_ownership_cells as legacy
import test_ownership_moments as coverage


class Monitor(legacy.Monitor):
    hostname = 'scc-ua1.scc.bu.edu'

    def report(self):
        report = super().report()
        report['host']['hostname'] = self.hostname
        return report


class OtherHost(Monitor):
    hostname = 'scc-uf1.scc.bu.edu'


def rewrite_request(path, change, body_change=lambda body: body):
    with path.open('rb') as stream:
        shape, args = q.case_header(stream)
        body = stream.read()
    args = change(args)
    shape[2] = len(args)
    path.write_bytes(b'FEREG_REPLAY_V1\n' + (' '.join(map(str, shape)) + '\n').encode()
                     + b''.join(str(len(arg)).encode() + b'\n' + arg + b'\n' for arg in args)
                     + body_change(body))


class Continuation(unittest.TestCase):
    def setUp(self):
        coverage.CoverageRunner.setUp(self)
        self.root = self.root.resolve()
        patch.object(oc.platform, 'system', return_value='Linux').start()
        self.plan.update(platform='Linux', observer='whole-host')
        self.plan['artifacts']['candidate']['revision'] = 'a' * 40
        self.plan['artifacts']['reference']['revision'] = oc.REFERENCE_REVISION
        for item in self.plan['cases']:
            if item['id'].endswith('-cluster'):
                rewrite_request(self.root / item['input'], lambda args: args + [b'account_cluster_nesting=0'])
                self.files[item['input']] = q.digest(self.root / item['input'])
            for name in (item['metadata'], item['oracle']['path']):
                self.real_write(self.root / name, {'synthetic': name})
                self.files[name] = q.digest(self.root / name)
        self.old_manifest = self.root / 'original-plan.json'
        self.real_write(self.old_manifest, self.plan)
        self.old_plan = copy.deepcopy(self.plan)
        self.old_directory = self.root / 'original-failed'

        def failed_fixture(command, log, timeout):
            if len(self.calls) == 45:
                self.calls.append(command)
                self.real_write(Path(command[-1]), {'return_code': 498, 'error': repair.ERROR, 'columns': []})
                log.write_text(repair.ERROR + '\n')
                raise ValueError(repair.ERROR)
            return self.execute(command, log, timeout)

        with self.assertRaisesRegex(ValueError, repair.ERROR):
            oc.run(self.old_manifest, self.old_directory, diagnostic=True,
                   execute=failed_fixture, monitor_factory=Monitor)
        self.assertEqual(len(self.calls), 46)
        old_path = self.old_directory / 'results.json'
        old = q.read(old_path)
        # Explicitly synthetic fixed-history fields, not a measured runtime.
        old['elapsed_s'] = 35.5
        self.real_write(old_path, old)
        self.real_write(self.old_directory / 'host/host.json', old['host'])
        self.old = copy.deepcopy(old)
        self.old_bytes = old_path.read_bytes()
        identity = routing.identity(self.old_plan)
        patch.object(repair, 'ORIGINAL_PLAN', identity).start()
        accounting = self.root / 'original-accounting.txt'
        accounting.write_text('jobnumber 8261951\nfailed 0\nexit_status 1\nslots 28\n'
                              'ru_wallclock 48\ngranted_pe mpi_28_tasks_per_node_u\nhostname scc-ua1\n')
        job_log = self.root / 'original-job.log'
        job_log.write_text('synthetic failed preflight: ' + repair.ERROR + '\n')
        self.plan = copy.deepcopy(self.old_plan)
        for item in self.plan['cases']:
            if item['id'].endswith('-cluster'):
                changed = self.root / ('corrected-' + item['input'])
                changed.write_bytes((self.root / item['input']).read_bytes())
                rewrite_request(changed, lambda args: [b'account_cluster_nesting=1' if a == b'account_cluster_nesting=0' else a for a in args])
                item['input'] = changed.name
                self.plan['files'][changed.name] = q.digest(changed)
        config = dict(plan=self.old_manifest.name, result=str(old_path.relative_to(self.root)),
                      accounting=accounting.name, job_log=job_log.name)

        def bind(value):
            path = (self.root / value).resolve()
            name = str(path.relative_to(self.root))
            self.plan['files'][name] = q.digest(path)
            return name

        self.plan['preflight_reuse'] = repair.freeze_amendment(config, bind, self.root / 'config.json')
        self.real_write(self.manifest, self.plan)
        self.calls.clear()
        self.started.clear()
        self.preflight = self.root / 'moments-preflight'

    def execute(self, command, log, timeout):
        return coverage.CoverageRunner.execute(self, command, log, timeout)

    def run_continuation(self, **kwargs):
        return oc.run(self.manifest, self.preflight, diagnostic=True,
                      execute=kwargs.pop('execute', self.execute),
                      monitor_factory=kwargs.pop('monitor_factory', Monitor), **kwargs)

    def test_exact_63_new_calls_preserve_45_original_objects_and_bound_budget(self):
        result = self.run_continuation()
        self.assertEqual(len(self.calls), 63)
        self.assertEqual([int(Path(c[-1]).stem.split('-')[1]) for c in self.calls], list(range(45, 108)))
        self.assertEqual((result['native_calls_started'], result['native_calls_reused'],
                          result['native_calls_executed_here']), (108, 45, 63))
        self.assertEqual(result['reused_preflight']['total_preflight_invocations'], 109)
        self.assertGreaterEqual(result['elapsed_s'], 36)
        self.assertLess(result['elapsed_s'], 1200)
        self.assertEqual((self.old_directory / 'results.json').read_bytes(), self.old_bytes)
        repair.verify_prefix(self.plan, self.manifest, self.preflight, result)
        moments.validate_preflight(self.plan, self.manifest, self.preflight / 'results.json')
        self.assertEqual(len(self.calls), 63)  # Offline validation executes nothing.
        self.assertEqual(len(self.plan['schedule']), 624)
        self.assertEqual(self.plan['schedule'], self.old_plan['schedule'])

    def test_first_and_mid_continuation_failures_retain_actual_attempt_counts(self):
        for fail_at in (1, 17):
            with self.subTest(new_call=fail_at):
                self.calls.clear()
                output = self.root / f'failure-{fail_at}'

                def fail(command, log, timeout):
                    value = self.execute(command, log, timeout)
                    if len(self.calls) == fail_at:
                        raise ValueError('synthetic continuation failure')
                    return value

                with self.assertRaisesRegex(ValueError, 'synthetic continuation failure'):
                    oc.run(self.manifest, output, diagnostic=True, execute=fail, monitor_factory=Monitor)
                result = q.read(output / 'results.json')
                self.assertEqual(result['status'], 'failed')
                self.assertEqual(result['native_calls_started'], 45 + fail_at)
                self.assertEqual(result['native_calls_executed_here'], fail_at)
                self.assertEqual(result['native_calls_reused'], 45)
                self.assertEqual(len(self.calls), fail_at)
                self.assertEqual(result['calls'][-1]['status'], 'started')
                self.assertTrue((output / result['calls'][-1]['log']).exists())
                self.assertEqual((self.old_directory / 'results.json').read_bytes(), self.old_bytes)

    def test_wrong_host_blocks_preflight_and_timing_before_any_new_callback(self):
        with self.assertRaisesRegex(ValueError, 'original physical host'):
            oc.run(self.manifest, self.root / 'wrong-preflight', diagnostic=True,
                   execute=self.execute, monitor_factory=OtherHost)
        self.assertEqual(self.calls, [])
        self.run_continuation()
        self.calls.clear()
        with self.assertRaisesRegex(ValueError, 'original physical host'):
            oc.run(self.manifest, self.root / 'wrong-timing', preflight_result=self.preflight / 'results.json',
                   execute=self.execute, monitor_factory=OtherHost)
        self.assertEqual(self.calls, [])
        self.assertEqual(q.read(self.root / 'wrong-timing/results.json')['native_calls_started'], 0)

    def test_old_failed_log_observer_and_retained_bank_tampering_blocks_execution(self):
        amendment = self.plan['preflight_reuse']
        reference = self.old['cells']['weak-robust-t1']['references']['B']
        paths = [self.root / amendment['failed_log'], self.root / amendment['failed_target'],
                 self.root / amendment['host_observations'],
                 (self.old_directory / reference['path']).with_suffix('.npz')]
        for index, path in enumerate(paths):
            with self.subTest(file=path.name):
                original = path.read_bytes()
                try:
                    path.write_bytes(original + b'changed')
                    with self.assertRaisesRegex(ValueError, 'changed'):
                        oc.run(self.manifest, self.root / f'tampered-old-{index}', diagnostic=True,
                               execute=self.execute, monitor_factory=Monitor)
                    self.assertEqual(self.calls, [])
                finally:
                    path.write_bytes(original)

    def test_cluster_rebinding_cannot_hide_an_extra_option_or_numeric_change(self):
        item = next(c for c in self.plan['cases'] if c['id'] == 'weak-cluster')
        source = self.root / item['input']
        original = source.read_bytes()
        mutations = [
            (lambda args: [b'solver=map' if a == b'solver=auto' else a for a in args], lambda body: body),
            (lambda args: args, lambda body: body.replace(b'3ff0000000000000', b'4000000000000000', 1)),
        ]
        for index, (args, body) in enumerate(mutations):
            with self.subTest(change=index):
                source.write_bytes(original)
                rewrite_request(source, args, body)
                self.assertNotEqual(source.read_bytes(), original)
                # Rebind the current manifest: the historical semantic seam,
                # rather than an ordinary stale-file digest, must reject it.
                self.plan['files'][item['input']] = q.digest(source)
                self.real_write(self.manifest, self.plan)
                with self.assertRaisesRegex(ValueError, 'cluster (amendment|numeric)'):
                    oc.run(self.manifest, self.root / f'tampered-cluster-{index}', diagnostic=True,
                           execute=self.execute, monitor_factory=Monitor)
                self.assertEqual(self.calls, [])


if __name__ == '__main__':
    unittest.main()
