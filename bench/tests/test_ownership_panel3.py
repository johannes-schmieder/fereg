"""Prospective binding and unchanged-arm checks; no native measurements."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownership_cells as oc
import ownership_panel3 as p3
import qualify as q


def detail():
    return dict(status='passed', oracle='passed', fingerprint='science', worker_signature='worker',
                model_signature='model', route_trace=[dict(installed='1')], route_hierarchy=[dict(levels=1)],
                cell_diagnostic=dict(bound=True), resource_checks=dict(memory_caps='passed', iteration_caps='passed'),
                action_resources=[dict(memory_bytes_limit=9000, memory_bytes_used=1000, iterations_used=21)],
                solver_trace_lines=['FEREG_SOLVER_TRACE|phase=two_fe_gates|grant=1'])


class Panel3(unittest.TestCase):
    def test_deferred_construction_stays_inside_installing_action(self):
        installed='FEREG_SOLVER_TRACE|phase=auto_schur_route|installed=1|refusal=None'
        refused=installed.replace('installed=1','installed=0')
        attempt='FEREG_CELLS|event=attempt|attempted=1|admitted=1'
        action=('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_two_gates_v1|iterations=2|native_limit=16000'
                '|memory_bytes=100|memory_limit=1000|final=1|certified=1')
        for cell in (attempt,attempt.replace('admitted=1','admitted=0')):
            self.assertEqual(p3.deferred_commitment('\n'.join([installed,cell,action])),
                             dict(installations=1,attempts=1))
        self.assertEqual(p3.deferred_commitment(refused+'\n'+action),dict(installations=0,attempts=0))
        for lines in ([attempt,installed,action], [installed,action,attempt],
                      [installed,action], [refused,attempt,action], [installed,installed,attempt],
                      [installed], [installed,attempt,attempt], [installed,attempt], []):
            with self.assertRaises(ValueError):p3.deferred_commitment('\n'.join(lines))
        for changed in (action.replace('memory_limit=1000','memory_limit=99'),
                        action.replace('certified=1','certified=0')):
            with self.assertRaises(ValueError):p3.deferred_commitment(changed)

    def test_deferred_action_coverage_and_raw_caps(self):
        rows=[dict(final=str(i),preset='routing_two_gates_v1',iterations_used=2,iterations_limit=16000,
                   memory_bytes_used=100,memory_bytes_limit=1000) for i in (0,1)]
        control=dict(action_resources=rows)
        p3.compare_actions(control,copy.deepcopy(control))
        for changed in ([],rows[:1],list(reversed(rows))):
            with self.assertRaises(ValueError):p3.compare_actions(dict(action_resources=changed),control)
        for key,value in [('preset','other'),('memory_bytes_used',1001),('iterations_used',16001)]:
            changed=copy.deepcopy(control);changed['action_resources'][0][key]=value
            with self.assertRaises(ValueError):p3.compare_actions(changed,control)

    def test_control_reuse_checks_unmodified_control_reachable_source(self):
        current={name:q.digest(path) for name,path in oc.sources().items()}
        path=self.root/'archived-controls.py'
        source=Path(oc.__file__).read_text()
        path.write_text(source.replace('def freeze(config_path, output):',
                                       'def freeze(config_path, output):\n    "Older non-control entrypoint."'))
        expected={**current,'ownership_cells':q.digest(path),'ownership_panel3':'old'}
        proof={'plan':{'execution_sources':expected}}
        p3.control_sources(proof,current,path)
        with self.assertRaisesRegex(ValueError,'source drift'):p3.control_sources(proof,current)
        path.write_text(source.replace('def controls_schedule():','def controls_schedule():\n    "Changed controls."'))
        expected['ownership_cells']=q.digest(path)
        with self.assertRaisesRegex(ValueError,'control-reachable'):p3.control_sources(proof,current,path)
        expected['qualify']='changed'
        with self.assertRaisesRegex(ValueError,'source drift'):p3.control_sources(proof,current,path)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / 'plan.json'
        cells = [f'{family}-t{w}' for family in oc.FAMILIES for w in (1, 4)]
        calls = [dict(detail(), cell=cell, arm=arm, phase='warmup') for cell in cells
                 for arm in ('B', 'O', 'S', 'O+S')]
        calls += [dict(detail(), cell=cell, arm=arm, phase='scored', block=block, mirror=mirror)
                  for block in range(6) for cell in cells for arm in ('B', 'O', 'S', 'O+S') for mirror in (0, 1)]
        self.prior = dict(status='complete', panel=2, platform='Linux', ownership='both',
                          inputs_sha256='a'*64, candidate_sha256='b'*64, diagnostic=False,
                          failures=[], exclusions=[], retries=0, native_calls_started=1248,
                          calls=calls, cells={c: {} for c in cells}, host=dict(host=dict(hostname='scc-ua1')))
        self.assessment = dict(status='verified', configuration_O='both', native_calls=0,
                               total_retained_native_calls=1344, source_verified_before_after=True,
                               bindings={}, independent_statistics=dict(cells_vs_ownership=[
                                   dict(cell=c, adverse_point_estimate=i == 0,
                                        mean_excess_s=.01 if i == 0 else -.01) for i, c in enumerate(cells)]))
        self.plan = dict(platform='Linux', ownership='both', inputs='inputs.json',
                         artifacts=dict(candidate=dict(path='plugin')),
                         files={'inputs.json': 'a'*64, 'plugin': 'c'*64},
                         panel2_result=dict(results='prior.json', assessment='assessment.json'))
        self.save()

    def save(self):
        q.write(self.root / 'prior.json', self.prior)
        self.assessment['bindings']['scored_result_sha256'] = q.digest(self.root / 'prior.json')
        q.write(self.root / 'assessment.json', self.assessment)
        for path in self.plan['panel2_result'].values():
            self.plan['files'][path] = q.digest(self.root / path)

    def test_changed_candidate_binds_preserved_success_and_regression(self):
        before, warmups = p3.prior_result(self.plan, self.manifest)
        self.assertEqual(before, self.prior)
        self.assertEqual(len(warmups), 96)
        p3.same_host(before, {'host': {'hostname': 'scc-ua1.scc.bu.edu'}})
        with self.assertRaisesRegex(ValueError, 'physical host'):
            p3.same_host(before, {'host': {'hostname': 'scc-ub2'}})

    def test_refuses_same_artifact_missing_trigger_failure_and_drift(self):
        self.plan['files']['plugin'] = 'b'*64
        with self.assertRaisesRegex(ValueError, 'changed candidate'):
            p3.prior_result(self.plan, self.manifest)
        self.plan['files']['plugin'] = 'c'*64
        self.assessment['independent_statistics']['cells_vs_ownership'][0]['adverse_point_estimate'] = False
        self.save()
        with self.assertRaisesRegex(ValueError, 'complete-command regression'):
            p3.prior_result(self.plan, self.manifest)
        self.assessment['independent_statistics']['cells_vs_ownership'][0]['adverse_point_estimate'] = True
        self.prior['failures'] = ['retained failure']
        self.save()
        with self.assertRaisesRegex(ValueError, 'completion/input'):
            p3.prior_result(self.plan, self.manifest)
        self.prior['failures'] = []
        self.save()
        (self.root / 'assessment.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'identity changed'):
            p3.prior_result(self.plan, self.manifest)

    def test_refuses_incomplete_or_duplicate_warmups_and_unverified_assessment(self):
        self.prior['calls'][1]['arm'] = 'B'
        self.save()
        with self.assertRaisesRegex(ValueError, 'warmup coverage'):
            p3.prior_result(self.plan, self.manifest)
        self.prior['calls'][1]['arm'] = 'O'
        self.assessment['status'] = 'failed'
        self.save()
        with self.assertRaisesRegex(ValueError, 'independent assessment'):
            p3.prior_result(self.plan, self.manifest)

    def test_existing_arms_exact_science_resources_and_gates(self):
        _, warmups = p3.prior_result(self.plan, self.manifest)
        for arm in ('B', 'O', 'O+eager'):
            value = detail()
            value['action_resources'][0]['memory_bytes_limit'] = 8000
            p3.compare_prior(warmups, 'weak-t1', arm, value)
            for key in ('fingerprint', 'worker_signature', 'model_signature', 'route_trace',
                        'route_hierarchy', 'cell_diagnostic', 'resource_checks'):
                changed = copy.deepcopy(value)
                changed[key] = 'changed'
                with self.assertRaisesRegex(ValueError, 'preserved'):
                    p3.compare_prior(warmups, 'weak-t1', arm, changed)
            value['action_resources'][0]['memory_bytes_used'] += 1
            with self.assertRaisesRegex(ValueError, 'action resources'):
                p3.compare_prior(warmups, 'weak-t1', arm, value)
            value = detail()
            value['solver_trace_lines'] = ['FEREG_SOLVER_TRACE|phase=two_fe_gates|grant=0']
            with self.assertRaisesRegex(ValueError, 'gate decisions'):
                p3.compare_prior(warmups, 'weak-t1', arm, value)
        # The new operator has its declared scoped mathematical comparisons.
        p3.compare_prior(warmups, 'weak-t1', 'O+deferred', {})


if __name__ == '__main__':
    unittest.main()
