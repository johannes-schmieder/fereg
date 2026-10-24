"""Frozen development runner tests; all generators, native work and math mocked."""
import copy
import hashlib
from pathlib import Path
import unittest
from unittest.mock import patch

import qualify as q
import routing
import two_fe_policy_development as development
import test_qualify
from test_cmgschur import case_bytes


def route(*, installed=True, iterations=16):
    result = dict(phase='auto_schur_route', checkpoint='16', samples='1',
                  iterations=str(iterations), installed=str(int(installed)),
                  refusal='None' if installed else 'Some(MemoryBudget)')
    if installed:
        result.update(terminal='Direct', levels='1', hierarchy_vertices='4',
                      hierarchy_nonzeros='10', repeats='1', terminal_factor='1',
                      terminal_factor_dimension='4', terminal_factor_nonzeros='10',
                      application='scalar', parallel_operators='0', batch_columns='1')
    return result


def decision(role='Packed', grant=1):
    return ('FEREG_SOLVER_TRACE|phase=two_fe_gates|event=decision|local=16'
            f'|role={role}|grant={grant}|terminal=1|proxy_entries=0|bank_bytes=0')


def build(installed=False):
    fields = route(installed=installed)
    fields['checkpoint'] = '0'
    return 'FEREG_SOLVER_TRACE|' + '|'.join(f'{k}={v}' for k, v in fields.items())


CORRECTION = ('FEREG_SOLVER_TRACE|phase=two_fe_gates|event=correction'
              '|grant=0|terminal=1|proxy_entries=0|bank_bytes=0')
ACTION = ('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_two_gates_v1'
          '|iterations=40|native_limit=100|memory_bytes=10|memory_limit=100|final=0|certified=1')


class GateTrace(unittest.TestCase):
    def diagnostics(self, log):
        action = ACTION.replace('final=0', 'final=1') if 'role=Final' in log else ACTION
        return development.resource_diagnostics(log + '\n' + action, 'candidate', '1'*40)

    def test_direct_short_retained_refused_and_terminal_episodes(self):
        for log in ('', decision('Final', 0), decision('Unknown', 0), CORRECTION,
                    decision()+'\n'+build(), decision()+'\n'+build(True)+'\n'+CORRECTION):
            result = self.diagnostics(log)
            self.assertIn('gate_events', result)
            self.assertTrue(routing.sha(development.semantic_signature(result)))

    def test_reopening_unsupported_grants_wrong_ages_outcomes_and_observer_work_fail(self):
        valid = decision()+'\n'+build(True)
        for log in (decision('Final'), decision('Unknown'), decision('Regressor'),
                    CORRECTION+'\n'+valid, valid+'\n'+decision('Final', 0),
                    decision(), build(), valid.replace('iterations=16', 'iterations=32'),
                    valid.replace('checkpoint=0', 'checkpoint=16'),
                    valid.replace('samples=1', 'samples=2'),
                    valid.replace('local=16', 'local=32'),
                    valid.replace('bank_bytes=0', 'bank_bytes=16'),
                    valid.replace('proxy_entries=0', 'proxy_entries=40'),
                    valid.replace('|terminal_factor=1', ''),
                    decision()+'\n'+build().replace('MemoryBudget', 'Cancelled'),
                    'FEREG_POLICY_LABEL|checkpoint|local=16'):
            with self.subTest(log=log), self.assertRaises((ValueError, KeyError)):
                self.diagnostics(log)

    def test_existing_caps_certification_and_preset_still_fail(self):
        for line in (ACTION.replace('iterations=40', 'iterations=101'),
                     ACTION.replace('memory_bytes=10', 'memory_bytes=101'),
                     ACTION.replace('certified=1', 'certified=0'),
                     ACTION.replace('routing_two_gates_v1', 'routing_incremental_two')):
            with self.subTest(line=line), self.assertRaises(ValueError):
                development.resource_diagnostics(line, 'candidate', '1'*40)

    def test_events_bind_to_completed_actions(self):
        valid = decision()+'\n'+build(True)
        for log in (decision()+'\n'+ACTION+'\n'+build(True)+'\n'+ACTION,
                    valid+'\n'+ACTION.replace('final=0', 'final=1'),
                    decision('Final', 0)+'\n'+ACTION,
                    valid+'\n'+ACTION.replace('iterations=40', 'iterations=15'),
                    valid, ACTION+'\n'+CORRECTION):
            with self.subTest(log=log), self.assertRaises(ValueError):
                development.resource_diagnostics(log, 'candidate', '1'*40)
        early = self.diagnostics(valid)
        later = self.diagnostics(ACTION+'\n'+valid)
        self.assertEqual(early['gate_events'][0]['action_ordinal'], 0)
        self.assertEqual(later['gate_events'][0]['action_ordinal'], 1)
        self.assertNotEqual(development.semantic_signature(early), development.semantic_signature(later))


class DevelopmentRunner(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.meta.update(absorbed_terms=2, categorical_intercepts=True)
        self.config = dict(platform='Darwin', harness='harness', artifacts={}, cases=[])
        for role, request in development.ARMS.items():
            (self.root / role).write_bytes(('synthetic artifact '+role).encode())
            self.config['artifacts'][role] = dict(path=role, request=request, build='synthetic',
                revision=development.ORIGINAL if role == 'original' else development.INCUMBENT if role == 'incumbent' else '1'*40,
                families=[] if role == 'original' else ['many'] if role == 'incumbent' else ['many', 'two'])
        q.write(self.root/'oracle.json', {'arrays': {'file': 'oracle.npz'}})
        (self.root/'oracle.npz').write_bytes(b'bound synthetic oracle')
        for family in development.FAMILIES:
            for memory in ('fast', 'low'):
                name = f's2026104201-{family}-{memory}'
                source = self.root/(name+'.case')
                source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=auto',
                    b'preconditioner=auto', f'memory={memory}'.encode()]))
                q.write(source.with_suffix('.json'), dict(seed=2026104201, family=family,
                    body_sha256=hashlib.sha256(family.encode()).hexdigest(), small=False,
                    random_streams=development.streams(2026104201, family), generator_sha256='a'*64,
                    generator_sources_sha256='b'*64))
                self.config['cases'].append(dict(id=name, family=family, input=source.name,
                    oracle=dict(kind='inactive') if family == 'inactive-columns' else dict(kind='routing', path='oracle.json'),
                    memory_limit_bytes=None))
        self.manifest = self.root/'plan.json'
        patch.object(development, 'validate_case').start()
        self.compatibility = patch.object(q.science, 'compatibility').start()
        test_qualify.RecoveryMonitor.hits = []
        test_qualify.RecoveryMonitor.quiet_failure = False
        patch('qualify_scc.MacMonitor', test_qualify.RecoveryMonitor).start()

    def plan(self):
        config = self.root/'config.json'
        q.write(config, self.config)
        return development.prepare(config, self.manifest)

    def execute(self, command, log, timeout):
        self.assertNotIn('FEREG_PHASE_TIMING=1', command)
        result = test_qualify.Qualification.execute(self, command, log, timeout)
        role = Path(command[-2]).stem
        traced = 'FEREG_SOLVER_TRACE=1' in command
        self.assertEqual(traced, Path(command[-1]).name.startswith('warmup-'))
        from native_io import case_header
        with Path(command[-2]).open('rb') as stream:
            _, arguments = case_header(stream)
        fields = dict(a.decode().split('=', 1) for a in arguments[1:])
        if role == 'candidate':
            self.assertEqual(fields['inject'], 'routing_two_gates_v1')
        else:
            self.assertNotIn('inject', fields)
        if traced and role == 'candidate':
            with log.open('a') as out:
                out.write(decision()+'\n'+build()+'\n'+ACTION+'\n')
        return result

    def test_frozen_full_schedule_keeps_untraced_times_and_protected_comparisons(self):
        manifest = self.plan()
        self.assertEqual(manifest['mac_cell_policy']['new_call_cap'], 432)
        self.assertEqual(manifest['mac_cell_policy']['max_cell_retries'], 0)
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record.get('failures'))
        self.assertEqual((len(record['cases']), len(self.calls)), (48, 432))
        self.assertEqual(self.oracle.call_count, 144)
        self.assertEqual(self.compatibility.call_count, 96)
        for cell in record['cases']:
            self.assertEqual((len(cell['warmups']), len(cell['blocks'][0]['calls'])), (3, 6))
            self.assertEqual(set(cell['blocks'][0]['comparisons']), {'original', 'incumbent'})
            self.assertEqual({c['seconds'] for c in cell['blocks'][0]['calls']}, {.01})
            self.assertTrue(all('gate_events' not in c for c in cell['blocks'][0]['calls']))
        summary = q.report(self.manifest, self.output)
        self.assertEqual(summary['status'], 'diagnostic_complete', summary['errors'])
        self.assertIs(summary['activation_qualified'], False)
        self.assertEqual((summary['native_calls'], len(summary['comparisons'])), (432, 96))
        self.assertEqual(summary['development_result_sha256'], q.digest(self.output/'run-0-0/results.json'))
        self.assertEqual(summary['policy_binding'], manifest['policy_binding'])
        self.assertEqual(manifest['files'][manifest['policy_binding']['fit']], development.FIT_SHA256)
        self.assertEqual(manifest['files'][manifest['policy_binding']['spec']], q.digest(Path(development.ROOT/'docs/two_fe_auto_gates.md')))
        self.assertEqual({r['status'] for r in summary['comparisons']}, {'inconclusive'})
        before = len(self.calls)
        with self.assertRaisesRegex(ValueError, 'diagnostic cannot resume'):
            q.run(self.manifest, self.output, resume=True, execute=self.execute)
        self.assertEqual(len(self.calls), before)

    def test_changed_schedule_baseline_or_alias_is_rejected_before_calls(self):
        value = dict(self.config, **development.fixed('Darwin'))
        value['policy_binding'] = development.selected_binding()
        development.validate(value)
        mutations = [('blocks', 2), ('seed', 2026105201), ('threads', [1, 4]),
                     ('budget_s', 14401), ('timeout_s', 121), ('recovery', {}), ('continuation', {})]
        for field, changed in mutations:
            with self.subTest(field=field), self.assertRaises(ValueError):
                development.validate({**value, field: changed})
        for role in ('original', 'incumbent'):
            bad = copy.deepcopy(value)
            bad['artifacts'][role]['revision'] = '2'*40
            with self.assertRaisesRegex(ValueError, 'baselines'):
                development.validate(bad)
        bad = copy.deepcopy(value)
        bad['files'] = {role: 'a'*64 for role in development.ARMS}
        bad['files'].update({bad['policy_binding']['fit']: development.FIT_SHA256,
                             bad['policy_binding']['spec']: 'a'*64})
        with self.assertRaisesRegex(ValueError, 'alias'):
            development.validate(bad)
        self.assertEqual(self.calls, [])

    def test_fit_and_wire_drift_fail_before_calls(self):
        value = dict(self.config, **development.fixed('Darwin'), policy_binding=development.selected_binding())
        for field in ('fit_sha256', 'wire_sha256', 'wire_hex'):
            bad = copy.deepcopy(value)
            bad['policy_binding'][field] = '0' * len(bad['policy_binding'][field])
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'binding|wire'):
                development.validate(bad)
        with patch.object(development, 'digest', return_value='0'*64), self.assertRaisesRegex(ValueError, 'fit changed'):
            development.selected_binding()
        self.assertEqual(self.calls, [])

    def test_missing_results_report_incomplete_without_digest(self):
        self.plan()
        summary = q.report(self.manifest, self.output)
        self.assertEqual(summary['status'], 'needs_discussion')
        self.assertIsNone(summary['development_result_sha256'])
        self.assertIs(summary['activation_qualified'], False)
        self.assertTrue(summary['errors'])

    def test_gate_decision_drift_between_workers_stops_without_replacement(self):
        self.plan()
        def changed(command, log, timeout):
            result = self.execute(command, log, timeout)
            from native_io import case_header
            with Path(command[-2]).open('rb') as stream:
                _, arguments = case_header(stream)
            if Path(command[-1]).name == 'warmup-candidate.json' and b'threads=4' in arguments:
                log.write_text('NATIVE_SECONDS=0.010000\n'+decision('Final', 0)+'\n'+ACTION.replace('final=0', 'final=1')+'\n')
            return result
        record = q.run(self.manifest, self.output, execute=changed)
        self.assertEqual(record['status'], 'blocked')
        self.assertIn('cross-worker gate decisions', record['failures'][-1]['error'])
        self.assertFalse(record['retry_cells'])
        self.assertEqual(len(self.calls), 12)


if __name__ == '__main__':
    unittest.main()
