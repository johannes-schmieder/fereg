"""Frozen D4 protocol: no native calls, builds, cluster access or timing."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compact_cells as c
from test_compact_cells import case_bytes


class EngineProtocol(unittest.TestCase):
    def plan(self):
        cells = [{'id': f'{family}-t{worker}', 'family': family, 'threads': worker}
                 for family in c.FAMILIES for worker in [1, 4]]
        return {'kind': c.ENGINE_KIND, 'cells': cells,
                'blocks': c.schedule([v['id'] for v in cells], arms=c.ENGINE_ARMS, limits=c.ENGINE_LIMITS)}

    def evidence(self):
        p = self.plan()
        rows = []
        for label, cid, arm, phase, block, repeat in c.engine_measurement_order(p):
            fingerprint = 'auto' if arm in ['baseline', 'fit_owned'] else 'engine' if arm in ['cells_e', 'cells_e_final'] else 's'
            row = dict(label=label, cell=cid, arm=arm, phase=phase, block=block, repeat=repeat,
                       status='passed', seconds={'baseline': 1., 'fit_owned': 1., 'cells_s': .9,
                                                 'cells_e': .8, 'cells_e_final': .8}[arm],
                       science_sha256=fingerprint)
            row['solver_rows'] = [[0.] * 16, [0.] * 16]
            row['solver_rows'][0][2] = 40
            row['solver_rows'][0][9] = .07 if arm == 'cells_e_final' else .1
            if phase == 'diagnostic':
                trace = {'phase': 'auto_cells_e' if arm == 'cells_e_final' else 'auto_cells_s',
                         'preset': 'routing_' + arm, 'used_operator': '1'}
                row['phase_diagnostics'] = {'solver_trace': [trace, {'phase': 'routing_action',
                    'preset': 'routing_' + arm, 'final': '0', 'certified': '1', 'iterations': '40'}]}
            rows.append(row)
        science = {}
        measurement = dict(kind=c.ENGINE_KIND + '_measurement', status='complete',
                           plan_sha256=c.routing.identity(p), science_sha256=c.routing.identity(science),
                           records=rows, files=[])
        return p, science, measurement

    def gate(self, evidence=None):
        with patch.object(c, 'validate'), patch.object(c, 'validate_science'):
            return c.engine_gate(*(evidence or self.evidence()))

    def test_fixed_schedule_counts_and_all_ten_historical_controls(self):
        p = self.plan()
        self.assertEqual(c.contract(p), (c.ENGINE_ARMS, c.ENGINE_LIMITS))
        self.assertEqual(c.ENGINE_ARMS, ['baseline', 'cells_s', 'cells_e', 'fit_owned', 'cells_e_final'])
        self.assertEqual(c.ENGINE_LIMITS['seed'], 2026101742)
        self.assertEqual(len(c.control_arms(p)), 10)
        self.assertIn('cells_s2', c.control_arms(p))
        order = c.engine_measurement_order(p)
        self.assertEqual(len(order), 1920)
        self.assertEqual([sum(v[3] == phase for v in order) for phase in ['warmup', 'measure', 'diagnostic']],
                         [120, 1440, 360])
        for cell in p['cells']:
            orders = [[r['arm'] for r in block if r['cell'] == cell['id']] for block in p['blocks']]
            for arm in c.ENGINE_ARMS:
                counts = [sum(order[position] == arm for order in orders) for position in range(5)]
                self.assertLessEqual(max(counts) - min(counts), 1)

    def test_exact_injections_and_existing_injection_rejection(self):
        for arm in c.ENGINE_ARMS:
            with self.subTest(arm=arm), tempfile.TemporaryDirectory() as directory:
                source, target = Path(directory) / 'source', Path(directory) / 'target'
                source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=auto', b'preconditioner=auto']))
                c.routing.prepare(source, target, arm, 4)
                c.validate_engine_request(target, arm, 4)
                with self.assertRaises(ValueError):
                    c.validate_engine_request(target, arm, 1)
                if arm != 'baseline':
                    with self.assertRaises(ValueError):
                        c.validate_engine_request(target, 'baseline', 4)
                    with self.assertRaisesRegex(ValueError, 'already contains'):
                        c.routing.prepare(target, Path(directory) / 'reinjected', arm, 4)

    def test_fixed_finalist_full_solve_gates_and_both_exactness_pairs(self):
        result = self.gate()
        self.assertEqual(result['status'], 'eligible')
        self.assertEqual(result['finalist'], 'cells_e_final')
        self.assertEqual(result['counts']['finalist_vs_auto']['material_gains'], 24)
        for arm in ['fit_owned', 'cells_e_final']:
            p, s, m = self.evidence()
            for row in m['records']:
                if row['arm'] == arm and row['cell'] == 'weak-t1':
                    row['science_sha256'] = 'consistent-ownership-drift'
            with self.subTest(arm=arm), self.assertRaisesRegex(ValueError, 'ownership'):
                self.gate((p, s, m))
        for arm in ['fit_owned', 'cells_e_final']:
            p, s, m = self.evidence()
            for row in m['records']:
                if row['arm'] == arm and row['cell'] == 'weak-t1' and row['phase'] == 'measure':
                    row['seconds'] = 1.06
            self.assertEqual(self.gate((p, s, m))['status'], 'ineligible')
        for gains, reference in [(17, 'baseline'), (2, 'cells_s')]:
            p, s, m = self.evidence()
            exempt = {cell['id'] for cell in p['cells'][:gains]}
            for row in m['records']:
                if row['arm'] == 'cells_e_final' and row['cell'] not in exempt:
                    row['seconds'] = 1. if reference == 'baseline' else .9
            self.assertEqual(self.gate((p, s, m))['status'], 'ineligible')

    def test_missing_reordered_duplicate_and_invalid_rows_cannot_pass(self):
        for mutation in ['missing', 'order', 'duplicate', 'science', 'nan', 'zero', 'failed']:
            p, s, m = self.evidence()
            if mutation == 'missing': m['records'].pop()
            elif mutation == 'order': m['records'][0], m['records'][1] = m['records'][1], m['records'][0]
            elif mutation == 'duplicate': m['records'][-1] = copy.deepcopy(m['records'][-2])
            elif mutation == 'science': m['records'][-1]['science_sha256'] = 'different'
            elif mutation == 'nan': m['records'][-1]['seconds'] = float('nan')
            elif mutation == 'zero': m['records'][-1]['seconds'] = 0.
            else: m['records'][-1]['status'] = 'failed'
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): self.gate((p, s, m))

    def test_first_solver_gate_uses_s_iterations_union_decisions_and_all_settings(self):
        for cid, candidate_time, passed in [('weak-t4', .08, True), ('weak-t4', .080001, False),
                                            ('weak-t1', .105, True), ('weak-t1', .105001, False)]:
            p, s, m = self.evidence()
            for row in m['records']:
                if (row['cell'], row['arm'], row['phase']) == (cid, 'cells_e_final', 'measure'):
                    row['solver_rows'][0][9] = candidate_time
                if (row['cell'], row['arm'], row['phase']) == (cid, 'cells_e_final', 'diagnostic'):
                    row['phase_diagnostics']['solver_trace'][0]['used_operator'] = '0'
            got = self.gate((p, s, m))
            self.assertEqual(got['status'], 'eligible' if passed else 'ineligible')
        for field in ['iteration', 'decision', 'missing']:
            p, s, m = self.evidence()
            row = next(r for r in m['records'] if (r['cell'], r['arm'], r['phase']) == ('easy-t4', 'cells_s', 'diagnostic'))
            if field == 'iteration': row['solver_rows'][0][2] = 30
            elif field == 'decision': row['phase_diagnostics']['solver_trace'][0]['used_operator'] = '0'
            else: row['phase_diagnostics']['solver_trace'] = []
            with self.subTest(field=field), self.assertRaises(ValueError): self.gate((p, s, m))

    def test_slow_scored_first_solver_cannot_hide_behind_fast_diagnostics(self):
        p, s, m = self.evidence()
        for row in m['records']:
            if row['cell'] == 'weak-t4' and row['arm'] == 'cells_e_final':
                row['solver_rows'][0][9] = .001 if row['phase'] == 'diagnostic' else .09
        got = self.gate((p, s, m))
        self.assertEqual(got['status'], 'ineligible')
        timer = next(v for v in got['first_solver_comparisons'] if v['cell'] == 'weak-t4')
        self.assertEqual(timer['finalist_median_seconds'], .09)
        self.assertEqual(len(timer['per_fit_seconds']['cells_e_final']), 12)
        self.assertFalse(timer['passed'])
        # Diagnostic timer inflation cannot fail a passing scored panel either.
        p, s, m = self.evidence()
        for row in m['records']:
            if row['arm'] == 'cells_e_final' and row['phase'] == 'diagnostic':
                row['solver_rows'][0][9] = 10.
        self.assertEqual(self.gate((p, s, m))['status'], 'eligible')

    def test_trace_mixed_batches_optional_stage_and_fallback(self):
        arm = 'cells_e_final'
        prefix = f'FEREG_SOLVER_TRACE|phase=auto_cells_e|preset=routing_{arm}|cells=10|rhs='
        log = prefix + '4|lanes=1|used_operator=0\n' + prefix + '1|lanes=1|used_operator=1\n'
        twin = f'FEREG_SOLVER_TRACE|phase=auto_cells_twin|preset=routing_{arm}|installed=0|refusal=Some(Work)|work=8|peak_bytes=128\n'
        self.assertEqual(len(c.routing_run.engine_trace_diagnostics(log, arm)), 2)
        self.assertEqual(len(c.routing_run.engine_trace_diagnostics(twin + log, arm)), 3)
        for bad in [log.replace('lanes=1', 'lanes=4'), log.replace('routing_' + arm, 'routing_cells_e'),
                    twin + twin + log, twin.replace('installed=0', 'installed=1') + log,
                    log.replace('rhs=4', 'rhs=0'), log.replace('cells=10', 'cells=0')]:
            with self.assertRaises(ValueError): c.routing_run.engine_trace_diagnostics(bad, arm)
        with self.assertRaises(ValueError): c.routing_run.engine_trace_diagnostics(log, 'fit_owned')
        self.assertEqual(c.routing_run.engine_trace_diagnostics('', 'fit_owned'), [])

    def test_zero_iteration_absence_and_final_residual_do_not_fake_projection_use(self):
        action = {'phase': 'routing_action', 'preset': 'routing_cells_e_final', 'final': '0',
                  'certified': '1', 'iterations': '0'}
        decision = {'phase': 'auto_cells_e', 'preset': 'routing_cells_e_final', 'used_operator': '1'}
        traces = [action, decision, {**action, 'final': '1', 'iterations': '10'}]
        self.assertFalse(c.engine_projection_uses_cells(traces, 'cells_e_final'))
        self.assertTrue(c.engine_projection_uses_cells([decision, {**action, 'iterations': '10'}], 'cells_e_final'))
        with self.assertRaisesRegex(ValueError, 'nonzero projection'):
            c.engine_projection_uses_cells([{**action, 'iterations': '1'}], 'cells_e_final')
        log = 'FEREG_SOLVER_TRACE|' + '|'.join(f'{key}={value}' for key, value in action.items()) + '\n'
        self.assertEqual(c.routing_run.engine_trace_diagnostics(log, 'cells_e_final'), [])
        with self.assertRaisesRegex(ValueError, 'nonzero-iteration'):
            c.routing_run.engine_trace_diagnostics(log.replace('iterations=0', 'iterations=1'), 'cells_e_final')
        resource = c.routing_run.resource_diagnostics('', 'fit_owned', False, {})
        self.assertEqual(resource['resource_checks']['setup_caps'], 'not_applicable')

    def test_prior_controls_bind_the_s2_artifact_and_refuse_rebinding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            harness, plugin = root / 'harness', root / 's2-plugin'
            harness.write_text('harness'); plugin.write_text('frozen-S2')
            inputs = {}
            source = root / 'source'
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=auto', b'preconditioner=auto']))
            for arm in c.ENGINE_CONTROLS:
                target = root / arm
                c.routing.prepare(source, target, arm, 1); inputs[arm] = c.ref(target)
            old = dict(id='weak-t1', family='weak', threads=1, inputs={arm: inputs[arm] for arm in c.ARMS})
            parent = root / 'parent.json'; c.save(parent, c.routing.seal({'cells': [old]}))
            prior = dict(kind=c.ADMISSION_KIND, harness=c.ref(harness), plugin=c.ref(plugin),
                         cells=[{**old, 'inputs': inputs}], frozen_files={})
            prior['frozen_files'] = {item['path']: item['sha256'] for item in [prior['harness'], prior['plugin'], *inputs.values()]}
            previous = root / 'prior.json'; c.save(previous, c.routing.seal(prior))
            plan = dict(kind=c.ENGINE_KIND, harness=prior['harness'], parent_plan=c.ref(parent), prior_admission_plan=c.ref(previous))
            with patch.object(c, 'validate_contract'):
                bindings = c.engine_control_bindings(plan)
                self.assertEqual(bindings[str(plugin.resolve())], c.digest(plugin))
                for arm in c.ENGINE_CONTROLS:
                    self.assertEqual(c.control_parent(plan, {'plugin': 'old-S'}, arm)['plugin'], prior['plugin'])
                plugin.write_text('rebound-S')
                with self.assertRaisesRegex(ValueError, 'frozen file changed'): c.engine_control_bindings(plan)

    def test_confirmation_precondition_blocks_before_any_candidate(self):
        with patch.object(c, 'validate'), patch.object(c, 'native') as native:
            with self.assertRaisesRegex(ValueError, 'frozen C4'):
                c.science(self.plan(), Path('/unused'), smoke=True)
            native.assert_not_called()


if __name__ == '__main__': unittest.main()
