"""Family-aware policy evidence through the real qualification runner/report."""
import copy
from pathlib import Path
import unittest
from unittest.mock import patch

import qualify as q
import routing
import test_qualify
from test_two_fe_policy_development import ACTION, build, decision


class PolicyEvidence(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.config['scope'] = 'two'
        self.config['artifacts']['incumbent']['families'] = ['many']
        self.config['artifacts']['candidate'].update(request=routing.GATES, families=['many', 'two'])
        self.presets = {'candidate': 'routing_two_gates_v1'}
        self.meta['absorbed_terms'] = 2
        patch.object(q.science, 'compatibility').start()

    def plan(self):
        for role, artifact in self.config['artifacts'].items():
            (self.root / role).write_bytes(('distinct artifact ' + role).encode())
            artifact['path'] = role
        return test_qualify.Qualification.plan(self)

    def execute(self, command, log, timeout):
        resources = test_qualify.Qualification.execute(self, command, log, timeout)
        role = Path(command[-4]).name
        if 'FEREG_SOLVER_TRACE=1' in command and role in self.presets:
            preset = self.presets[role]
            trace = ACTION.replace('routing_two_gates_v1', preset)
            if preset == 'routing_two_gates_v1':
                trace = decision() + '\n' + build() + '\n' + trace
            with log.open('a') as stream:
                stream.write(trace + '\n')
        return resources

    def completed(self):
        self.plan()
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'complete', record['failures'])
        report = q.report(self.manifest, self.output)
        self.assertEqual(report['status'], 'smoke_complete', report['errors'])
        self.assertTrue(all(c['timing_field'] == 'NATIVE_SECONDS' for cell in record['cases']
                            for c in [*cell['warmups'], *(c for block in cell['blocks'] for c in block['calls'])]))
        return record

    def public(self):
        self.config['artifacts']['hidden'] = copy.deepcopy(self.config['artifacts']['candidate'])
        self.config['artifacts']['candidate']['request'] = 'auto'
        self.presets['hidden'] = 'routing_two_gates_v1'

    def test_hidden_two_grant_refusal_and_report_require_gate_evidence(self):
        record = self.completed()
        warmups = [next(c for c in cell['warmups'] if c['execution'] == 'candidate')
                   for cell in record['cases']]
        self.assertEqual(warmups[0]['gate_events'][0]['grant'], '1')
        self.assertEqual(warmups[0]['route_trace'][0]['installed'], '0')
        for mutate in (lambda w: w.pop('gate_events'),
                       lambda w: w['gate_events'][0].update(action_ordinal=1)):
            changed = copy.deepcopy(record)
            candidate = next(c for c in changed['cases'][1]['warmups'] if c['execution'] == 'candidate')
            mutate(candidate)
            q.write(self.output / 'run-0-0/results.json', changed)
            self.assertEqual(q.report(self.manifest, self.output)['status'], 'needs_discussion')

    def test_hidden_many_uses_accepted_many_preset(self):
        self.meta['absorbed_terms'] = 4
        self.presets = dict(candidate='routing_incremental_many', incumbent='routing_incremental_many')
        record = self.completed()
        self.assertTrue(all('gate_events' not in c for cell in record['cases'] for c in cell['warmups']))

    def test_hidden_outside_scope_needs_no_gate_fields(self):
        self.meta['absorbed_terms'] = 0
        self.presets = {}
        self.completed()

    def test_public_candidate_uses_hidden_gate_binding_without_changing_incumbent(self):
        self.public()
        record = self.completed()
        evidence = {c['execution']: c for c in record['cases'][0]['warmups']}
        self.assertIn('gate_events', evidence['candidate'])
        self.assertIn('gate_events', evidence['hidden'])
        self.assertNotIn('gate_events', evidence['incumbent'])

    def test_public_wrong_preset_stops_the_run(self):
        self.public()
        self.presets['candidate'] = 'routing_incremental_two'
        self.plan()
        record = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(record['status'], 'blocked')
        self.assertIn('requested preset', record['failures'][-1]['error'])

    def test_hidden_gate_decisions_must_agree_across_workers(self):
        self.plan()
        def changed(command, log, timeout):
            resources = self.execute(command, log, timeout)
            if Path(command[-1]).name == 'warmup-candidate.json' and len(self.calls) > 15:
                log.write_text('NATIVE_SECONDS=0.010000\n' + decision('Final', 0) + '\n' +
                               ACTION.replace('final=0', 'final=1') + '\n')
            return resources
        record = q.run(self.manifest, self.output, execute=changed)
        self.assertEqual(record['status'], 'blocked')
        self.assertIn('cross-worker gate decisions', record['failures'][-1]['error'])

    def test_historical_requests_and_public_override_are_family_scoped(self):
        for request in (*routing.INCREMENTAL, *routing.REMAINING, routing.GATES):
            self.config['artifacts']['candidate']['request'] = request
            two = request if request in (*routing.REMAINING, routing.GATES) else 'incremental-two'
            self.assertEqual(q.resource_policy(self.config, 'candidate', self.meta), two)
            self.assertEqual(q.resource_policy(self.config, 'candidate', dict(self.meta, absorbed_terms=4)),
                             'incremental-many')
            self.assertIsNone(q.resource_policy(self.config, 'candidate', dict(self.meta, absorbed_terms=0)))
        self.public()
        self.assertEqual(q.resource_policy(self.config, 'candidate', self.meta), routing.GATES)
        self.assertIsNone(q.resource_policy(self.config, 'incumbent', self.meta))


class ScalarAccounting(unittest.TestCase):
    def test_scalar_spellings_do_not_relax_array_job_or_success_identity(self):
        template = ('jobnumber 123\ntaskid {}\nhostname scc-node.bu.edu\nproject welfgr\n'
                    'failed 0\nexit_status 0\nend_time done\n')
        for observed in ('0', 'undefined', '1', '2', '', 'bad'):
            for recorded in ('0', 'undefined', '1', '2', '', 'bad'):
                host = dict(job_id='123', task_id=recorded, hostname='scc-node')
                valid = (observed == recorded and observed in ('1', '2') or
                         observed in ('0', 'undefined') and recorded in ('0', 'undefined'))
                with self.subTest(observed=observed, recorded=recorded):
                    if valid:
                        q.accounting(template.format(observed), host)
                    else:
                        with self.assertRaises(ValueError):
                            q.accounting(template.format(observed), host)
        raw = template.format('undefined')
        for bad in (raw.replace('exit_status 0', 'exit_status 1'), raw.replace('failed 0', 'failed 1'),
                    raw.replace('scc-node.bu.edu', 'other'), raw.replace('jobnumber 123', 'jobnumber 124'),
                    raw.replace('project welfgr', 'project other'), raw + 'taskid undefined\n',
                    raw.replace('taskid undefined\n', ''), raw.replace('end_time done\n', '')):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                q.accounting(bad, dict(job_id='123', task_id='0', hostname='scc-node'))


if __name__ == '__main__':
    unittest.main()
