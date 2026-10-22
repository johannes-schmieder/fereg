"""Full frozen schedule through the real runner; every native call is fake."""
import copy
import hashlib
from pathlib import Path
import unittest
from unittest.mock import patch

import qualify as q
import routing
import two_fe_policy as policy
import two_fe_policy_calibrate as calibration
from native_io import case_header
import test_qualify
from test_cmgschur import case_bytes
from test_qualify import RecoveryMonitor
from test_two_fe_policy_labels import projection
from test_two_fe_policy_outcomes import route


def label(kind, fields):
    return 'FEREG_POLICY_LABEL|' + kind + '|' + '|'.join(
        f"{key}={'none' if value is None else value}" for key, value in fields.items())


class CalibrationRunner(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.meta.update(absorbed_terms=2, categorical_intercepts=True)
        self.config = dict(platform='Darwin', harness='harness', artifacts={}, cases=[])
        for role, request in policy.ARMS.items():
            self.config['artifacts'][role] = dict(path='plugin', revision='1'*40,
                build='synthetic-native', request=request, families=['two'])
        q.write(self.root/'oracle.json', {'arrays': {'file': 'oracle.npz'}})
        (self.root/'oracle.npz').write_bytes(b'bound synthetic oracle')
        for seed in policy.SEEDS:
            for family in policy.FAMILIES:
                for memory in ('fast', 'low'):
                    name = f's{seed}-{family}-{memory}'
                    source = self.root/(name+'.case')
                    source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=auto',
                                                   b'preconditioner=auto', f'memory={memory}'.encode()]))
                    q.write(source.with_suffix('.json'), dict(seed=seed, family=family,
                        body_sha256=hashlib.sha256(f'{seed}-{family}'.encode()).hexdigest(),
                        random_streams=policy.streams(seed, family), generator_sha256='a'*64))
                    self.config['cases'].append(dict(id=name, family=family, input=source.name,
                        oracle=dict(kind='routing', path='oracle.json'), memory_limit_bytes=None))
        self.manifest = self.root/'plan.json'
        # Tiny replay bytes and mathematics are synthetic. Scheduling, framing,
        # identities, logs, label parsing, run/report and collect remain real.
        patch.object(policy, 'validate_case').start()
        self.compatibility = patch.object(q.science, 'compatibility').start()
        RecoveryMonitor.hits = []
        RecoveryMonitor.quiet_failure = False
        patch('qualify_scc.MacMonitor', RecoveryMonitor).start()

    def plan(self):
        config = self.root/'config.json'
        q.write(config, self.config)
        return policy.prepare(config, self.manifest)

    def execute(self, command, log, timeout):
        self.assertIn('FEREG_SOLVER_TRACE=1', command)
        self.assertIn('FEREG_PHASE_TIMING=1', command)
        result = test_qualify.Qualification.execute(self, command, log, timeout)
        request = Path(command[-2])
        role = request.stem
        with request.open('rb') as stream:
            _, arguments = case_header(stream)
        fields = dict(value.decode().split('=', 1) for value in arguments[1:])
        self.assertEqual(fields['inject'], routing.policy(policy.ARMS[role])['injection'])
        workers, memory = int(fields['threads']), fields['memory']
        first = projection('packed', end=2, width=3) if memory == 'fast' else projection('outcome')
        final = projection('final', total=0, block=0)
        final['checkpoints'] = []
        actions = [first, final]
        if memory == 'low':
            middle = projection('regressor', end=2, width=2)
            middle['checkpoints'] = []
            middle['end'].update(steps=0, live=0)
            actions.insert(1, middle)
        first['checkpoints'][0]['workers'] = workers
        if role == 'never':
            first['checkpoints'].append(dict(first['checkpoints'][0], local=32, stamp_ns=20))
        else:
            age = 16 if role == 'force16' else 32
            first['checkpoints'][0].update(local=age, build=1)
            first['end'].update(elapsed_ns=125, build_ns=30, restart_ns=35,
                                startup_ns=20, continuation_ns=30, installed=1)
        lines = ['NATIVE_SECONDS=0.010000']
        for action in actions:
            lines.extend(label('checkpoint', point) for point in action['checkpoints'])
            if action['end']['installed']:
                outcome = route(iterations=age)
                outcome['checkpoint'] = str(age)
                lines.append('FEREG_SOLVER_TRACE|' + '|'.join(f'{key}={value}' for key, value in outcome.items()))
            end = action['end']
            lines.append('FEREG_SOLVER_TRACE|phase=routing_action'
                f"|preset={fields['inject']}|iterations={end['steps']}|native_limit=100"
                f"|memory_bytes=10|memory_limit=100|final={end['final']}|certified=1")
            lines.append(label('action_end', end))
        log.write_text('\n'.join(lines)+'\n')
        return result

    def assert_no_retry(self, record):
        self.assertEqual(record['status'], 'blocked')
        self.assertFalse(record['retry_cells'])
        self.assertEqual(record['native_calls_started'], len(self.calls))
        before = len(self.calls)
        with self.assertRaisesRegex(ValueError, 'diagnostic cannot resume'):
            q.run(self.manifest, self.output, resume=True, execute=self.execute)
        self.assertEqual(len(self.calls), before)
        summary = q.report(self.manifest, self.output)
        self.assertEqual(summary['status'], 'needs_discussion')
        with self.assertRaisesRegex(ValueError, 'incomplete, failed or unbound'):
            calibration.collect(self.manifest, self.output)

    def test_full_864_call_plan_run_report_collect_and_result_binding(self):
        manifest = self.plan()
        self.assertEqual(manifest['mac_cell_policy']['new_call_cap'], 864)
        self.assertEqual(manifest['mac_cell_policy']['max_cell_retries'], 0)
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record.get('failures'))
        self.assertEqual(len(record['cases']), 96)
        self.assertEqual(len(self.calls), 864)
        self.assertEqual(self.oracle.call_count, 288)
        self.assertEqual(self.compatibility.call_count, 192)
        all_calls = [call for cell in record['cases'] for call in
                     [*cell['warmups'], *cell['blocks'][0]['calls']]]
        self.assertEqual(len(all_calls), 864)
        for cell in record['cases']:
            self.assertEqual(len(cell['warmups']), 3)
            self.assertEqual(len(cell['blocks']), 1)
            self.assertEqual(len(cell['blocks'][0]['calls']), 6)
            self.assertEqual(set(cell['blocks'][0]['comparisons']), {'never->force16', 'never->force32'})
        self.assertTrue(all(call['policy_actions'] and call['action_resources'] for call in all_calls))
        self.assertTrue(all(call['seconds'] == .01 for call in all_calls))
        summary = q.report(self.manifest, self.output)
        self.assertEqual(summary['status'], 'diagnostic_complete', summary['errors'])
        self.assertIs(summary['activation_qualified'], False)
        self.assertEqual(summary['native_calls'], 864)
        self.assertEqual(len(summary['comparisons']), 192)
        self.assertEqual({row['reference'] for row in summary['comparisons']},
                         {'never->force16', 'never->force32'})
        result_path = self.output/'run-0-0/results.json'
        self.assertEqual(summary['calibration_result_sha256'], q.digest(result_path))
        good, missing, binding = calibration.collect(self.manifest, self.output)
        self.assertEqual(len(good), 192)
        self.assertFalse(missing)
        self.assertEqual(binding['result_sha256'], q.digest(result_path))
        self.assertEqual(len(binding['inputs']), 32)
        # Neither an incomplete cell nor an otherwise innocuous result mutation
        # may replace the exact object whose science/report checks completed.
        changed = copy.deepcopy(record)
        changed['cases'][0]['status'] = 'running'
        q.write(result_path, changed)
        with self.assertRaisesRegex(ValueError, 'actual calibration results'):
            calibration.collect(self.manifest, self.output)
        q.write(result_path, record)
        incomplete = q.read(self.output/'summary.json')
        incomplete['native_calls'] = 863
        q.write(self.output/'summary.json', incomplete)
        with self.assertRaisesRegex(ValueError, 'incomplete, failed or unbound'):
            calibration.collect(self.manifest, self.output)

    def test_scientific_failure_stops_after_warmups_without_retry(self):
        self.plan()
        self.compatibility.side_effect = ValueError('synthetic independent science failure')
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(len(self.calls), 3)
        self.assertEqual(record['failures'][-1]['kind'], 'scientific')
        self.assert_no_retry(record)

    def test_repeat_failure_stops_on_first_recorded_call_without_retry(self):
        self.plan()
        def changed(command, log, timeout):
            if len(self.calls) == 3:
                self.data[1, 0] += .01
            return self.execute(command, log, timeout)
        record = q.run(self.manifest, self.output, execute=changed)
        self.assertEqual(len(self.calls), 4)
        self.assertEqual(record['failures'][-1]['kind'], 'scientific')
        self.assertIn('fixed-worker repeat changed', record['failures'][-1]['error'])
        self.assert_no_retry(record)

    def test_exhausted_budget_stops_without_sleep_or_native_retry(self):
        self.plan()
        elapsed = [0.0]
        def expired(command, log, timeout):
            value = self.execute(command, log, timeout)
            elapsed[0] = 14401.0
            return value
        with patch.object(q.time, 'monotonic', side_effect=lambda: 100.0 + elapsed[0]):
            record = q.run(self.manifest, self.output, execute=expired)
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(record['failures'][-1]['kind'], 'budget')
        self.assertFalse(record['budget_compliant'])
        self.assert_no_retry(record)


if __name__ == '__main__':
    unittest.main()
