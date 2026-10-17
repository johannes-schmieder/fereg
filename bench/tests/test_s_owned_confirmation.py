"""C5 freezes fresh data and two comparisons within complete six-call units."""
import copy
from collections import Counter
from contextlib import ExitStack
from itertools import permutations
from pathlib import Path
import sys
import os
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import s_owned_confirmation as s
import engine_confirmation as old
import membership_confirmation_run as run
from test_membership_confirmation_run import sha
from test_membership_confirmation_run import record_fields

c = s.campaign


class SOwnedConfirmation(unittest.TestCase):
    def plan(self):
        cells = [dict(id=f"{spec['family']}-t{worker}", family=spec['family'], threads=worker,
                      inputs={arm: {} for arm in s.ARMS}) for spec in s.CASES for worker in [1, 4]]
        return dict(kind=s.KIND, arms=s.ARMS, cases=s.CASES, limits=s.LIMITS, oracle_caps=s.ORACLE_CAPS,
                    library_limits=c.LIBRARY_LIMITS, analysis=s.ANALYSIS, execution=s.EXECUTION,
                    cells=cells, blocks=s.schedule([cell['id'] for cell in cells]), prepared_wall_ns=1)

    def rows(self, plan):
        return [dict(label=label, cell=cid, arm=arm, phase=phase, block=block, position=position,
                     seconds={'baseline': 1., 'cells_s': .9, 'cells_s_owned': .88}[arm], status='passed', resources={},
                     science_sha256=sha(cid + ('auto' if arm == 'baseline' else 's')))
                for label, cid, arm, phase, block, position in run.measurement_order(plan)]

    def test_fresh_seeds_same_ten_shapes_and_balanced_complete_mirrors(self):
        plan = self.plan(); s.validate_contract(plan)
        self.assertEqual(s.CASES, old.CASES)
        self.assertEqual(s.ARMS, ['baseline', 'cells_s', 'cells_s_owned'])
        self.assertEqual((s.LIMITS['seed'], s.LIMITS['data_seed']), (2026101753, 2026101754))
        self.assertNotEqual(s.LIMITS['data_seed'], old.LIMITS['data_seed'])
        self.assertEqual(len(run.measurement_order(plan)), 1500)
        self.assertEqual(len(run.science_order(plan)), 120)
        self.assertEqual([s.EXECUTION[k] for k in ['warmup_calls', 'scientific_calls', 'scored_calls']], [60, 120, 1440])
        for cell in plan['cells']:
            orders = [slot['arms'] for block in plan['blocks'] for slot in block if slot['cell'] == cell['id']]
            self.assertTrue(all(order == list(reversed(order)) and Counter(order) == {arm: 2 for arm in s.ARMS} for order in orders))
            self.assertEqual(Counter(tuple(order[:3]) for order in orders), {p: 2 for p in permutations(s.ARMS)})
        for mutation in ['seed', 'case', 'order', 'caps', 'finalist', 'freeze']:
            changed = copy.deepcopy(plan)
            if mutation == 'seed': changed['limits']['data_seed'] = old.LIMITS['data_seed']
            elif mutation == 'case': changed['cases'][-1]['periods'] -= 1
            elif mutation == 'order': changed['blocks'][0][0]['arms'].reverse(); changed['blocks'][0].reverse()
            elif mutation == 'caps': changed['oracle_caps']['MAX_ROWS'] += 1
            elif mutation == 'finalist': changed['analysis']['finalist'] = 'cells_e_final'
            else: changed['prepared_wall_ns'] = 0
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): s.validate_contract(changed)

    def test_shared_generator_uses_c5_seed_without_mutating_c4(self):
        spec = s.CASES[0]
        values = [c.np.ones(spec['workers'] * spec['periods']) for _ in range(5)]
        with patch.object(c.cases, 'synthetic', return_value=values) as generate, patch.object(s, 'oracle_capacity'):
            s.columns(spec)
            self.assertEqual(generate.call_args.args[-1], 2026101754)
        self.assertEqual(old.LIMITS['data_seed'], 2026101744)
        self.assertEqual(old.ARMS, ['baseline', 'cells_e_final'])

    def test_two_comparisons_share_each_complete_unit_and_both_must_pass(self):
        plan = self.plan(); rows = self.rows(plan)
        got = run.summarize(plan, rows)
        self.assertEqual(got['status'], 'passed')
        self.assertEqual(len(got['settings']), 40)
        self.assertEqual(len(got['block_scores']), 480)
        for block in range(12):
            for cell in plan['cells']:
                scores = [score for score in got['block_scores'] if (score['block'], score['cell']) == (block, cell['id'])]
                self.assertEqual({v['reference'] for v in scores}, {'baseline', 'cells_s'})
                self.assertEqual(scores[0]['candidate_seconds'], scores[1]['candidate_seconds'])
        # Exactly three upper-tail failures exceed the tenth-order-statistic rule.
        for row in rows:
            if row['cell'] == 'weak480k-t4' and row['arm'] == 'cells_s' and row['phase'] == 'measure' and row['block'] >= 9:
                row['seconds'] = .7
        got = run.summarize(plan, rows)
        self.assertEqual(got['status'], 'failed')
        failed = [v for v in got['settings'] if not v['passed']]
        self.assertEqual([(v['cell'], v['reference']) for v in failed], [('weak480k-t4', 'cells_s')])
        self.assertEqual(run.summarize(plan, rows[:-1])['status'], 'incomplete')
        rows[0], rows[1] = rows[1], rows[0]
        with self.assertRaises(ValueError): run.summarize(plan, rows)

    def test_exact_s_pair_is_required_in_science_and_all_measurements(self):
        plan = self.plan(); science = []
        for cid, arm, repeat in run.science_order(plan):
            science.append(dict(cell=cid, arm=arm, repeat=repeat, label=f'science-{cid}-{arm}-{repeat}',
                                status='executed', oracle_status='passed', resources={},
                                science_sha256=sha(cid + ('auto' if arm == 'baseline' else 's')),
                                model_discrete_sha256=sha('discrete')))
        fingerprint = lambda rows: {row['cell'] + '/' + row['arm']: row['science_sha256'] for row in rows}
        run.check_science_records(plan, {'records': science, 'fingerprints': fingerprint(science)})
        for row in science:
            if row['cell'] == 'short2-t1' and row['arm'] == 'cells_s_owned': row['science_sha256'] = sha('changed')
        with self.assertRaisesRegex(ValueError, 'ownership'):
            run.check_science_records(plan, {'records': science, 'fingerprints': fingerprint(science)})
        rows = self.rows(plan); fingerprints = fingerprint(rows)
        run.validate_measurement_records(plan, rows, fingerprints)
        for phase in ['warmup', 'measure', 'all']:
            changed = copy.deepcopy(rows)
            for row in changed:
                if row['cell'] == 'short2-t1' and row['arm'] == 'cells_s_owned' and (phase == 'all' or row['phase'] == phase):
                    row['science_sha256'] = sha('changed')
            bound = fingerprint(changed) if phase == 'all' else fingerprints
            with self.subTest(phase=phase), self.assertRaises(ValueError): run.validate_measurement_records(plan, changed, bound)

    def test_driver_binds_c5_and_cannot_be_reused_for_c4(self):
        plan = self.plan()
        with tempfile.TemporaryDirectory() as directory:
            driver = run.freeze(c.routing.identity(plan), Path(directory) / 'driver.json', discovery_unopened=True, profile='s_owned')
            run.validate_driver(plan, driver)
            self.assertIn('bench/tests/test_s_owned_cells.py', driver['sources'])
            changed = copy.deepcopy(driver); changed['sources']['bench/tests/test_s_owned_cells.py'] = 'changed'
            with self.assertRaises(ValueError): run.validate_driver(plan, changed)
            with self.assertRaises(ValueError): run.validate_driver({**plan, 'kind': old.KIND}, driver)

    def test_c5_recomputes_d5_only_and_requires_both_earlier_freezes(self):
        plan = self.plan(); plan['discovery_plan'] = {'path': 'discovery'}
        discovery = {'kind': c.S_OWNED_KIND}
        smoke = dict(kind=c.S_OWNED_KIND + '_science', status='passed', smoke=True,
                     plan_sha256=c.routing.identity(discovery), job_id='1', host='science', slots=4, records=[{}], files=[])
        science = dict(job_id='1', host='science', slots=4)
        measurement = {'host': {'hostname': 'd5-host'}}
        values = dict(discovery=discovery, smoke=smoke, science=science, measurement=measurement, isolation_review={})
        evidence = {name: {'path': name} for name in ['smoke', 'science', 'measurement', 'science_accounting',
                                                     'measurement_accounting', 'isolation_review']}
        for mode in ['eligible', 'ineligible', 'old-finalist', 'late-plan', 'late-driver']:
            current = copy.deepcopy(plan); driver = {'frozen_wall_ns': 2}
            if mode == 'late-plan': current['prepared_wall_ns'] = 10
            if mode == 'late-driver': driver['frozen_wall_ns'] = 10
            gate = {'status': 'ineligible' if mode == 'ineligible' else 'eligible',
                    'finalist': 'cells_e_final' if mode == 'old-finalist' else 'cells_s_owned'}
            with self.subTest(mode=mode), patch.object(s, 'validate'), patch.object(run, 'validate_driver'), \
                 patch.object(c, 'checked', side_effect=lambda item: item['path']), \
                 patch.object(run, 'unsealed', side_effect=lambda path: values[path]), \
                 patch.object(run, 'check_record_files'), patch.object(c, 'read', return_value={}), \
                 patch.object(run, 'accounting'), patch.object(run, 'isolation_review'), \
                 patch.object(run, 'wall_window', return_value=(10, 20)), \
                 patch.object(c, 'validate_science') as smoke_validation, \
                 patch.object(c, 'engine_gate', side_effect=AssertionError('D4 gate must remain unused')), \
                 patch.object(c, 's_owned_gate', return_value=gate) as recompute:
                if mode == 'eligible':
                    self.assertEqual(run.admission(current, driver, evidence)['discovery_host'], 'd5-host')
                else:
                    with self.assertRaises(ValueError): run.admission(current, driver, evidence)
                recompute.assert_called_once_with(discovery, science, measurement)
                smoke_validation.assert_called_once_with(discovery, smoke, smoke=True)

    def test_same_host_and_failed_admission_block_before_native_calls(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(run, 'validate_admission'), \
             patch.object(run, 'validate_science'), patch.object(run, 'check_library_limits'), \
             patch.object(c.scc.discovery, 'host', return_value={'slots': 28, 'hostname': 'd5-host.scc.bu.edu'}), \
             patch.object(c, 'native') as native:
            with self.assertRaisesRegex(ValueError, 'distinct physical host'):
                run.measure(self.plan(), {}, {'discovery_host': 'd5-host'}, {}, Path(directory) / 'out')
            native.assert_not_called()
        with patch.object(run, 'validate_admission', side_effect=ValueError('D5 failed')), patch.object(c, 'native') as native:
            with self.assertRaisesRegex(ValueError, 'D5 failed'): run.science(self.plan(), {}, {}, Path('/unused'))
            native.assert_not_called()

    def test_ownership_failure_stops_on_first_warmup_pair_before_any_scored_call(self):
        plan = self.plan()
        observer = Mock(errors=[])
        host = {'slots': 28, 'affinity': list(range(28)), 'hostname': 'c5-host', 'job_id': '2'}
        calls = []
        def native(plan, cell, arm, output, label, **options):
            calls.append(label)
            return record_fields(cell, arm, label, output)
        def prerequisite(row, cell, receipt, **options):
            row['resources'] = {}
            return {'science_sha256': sha(row['arm']), 'seconds': 1.}
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            for owner, name, options in [
                (run, 'validate_admission', {}), (run, 'validate_science', {}),
                (c, 'native', {'side_effect': native}), (c, 'check_prerequisite', {'side_effect': prerequisite}),
                (c, 'Observers', {'return_value': observer}), (c, 'check_stable_configuration', {}),
                (c.scc.discovery, 'host', {'return_value': host}), (c.scc, 'configuration', {'return_value': ({}, {})}),
                (c.scc, 'check_configuration', {'return_value': 'queue'}), (c.scc, 'check_scheduler_snapshot', {}),
            ]:
                stack.enter_context(patch.object(owner, name, **options))
            stack.enter_context(patch.dict(os.environ, c.LIBRARY_LIMITS))
            result = run.measure(plan, {}, {'discovery_host': 'd5-host'}, {}, Path(directory) / 'measure')
        self.assertEqual(result['status'], 'failed')
        self.assertIn('ownership', result['failure'])
        self.assertEqual(calls, [f'warmup-short2-t1-{arm}' for arm in s.ARMS])


if __name__ == '__main__': unittest.main()
