"""C4 resource caps, exact profile binding, and untouched confirmation protocol."""
import copy
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import engine_confirmation as e
import membership_confirmation_run as run
import admission_confirmation as prior
c = e.campaign


class EngineConfirmation(unittest.TestCase):
    def plan(self):
        cells = [dict(id=f"{spec['family']}-t{worker}", family=spec['family'], threads=worker,
                      inputs={arm: {} for arm in e.ARMS}) for spec in e.CASES for worker in [1, 4]]
        return dict(kind=e.KIND, cases=e.CASES, arms=e.ARMS, limits=e.LIMITS, oracle_caps=e.ORACLE_CAPS,
                    library_limits=c.LIBRARY_LIMITS, analysis=e.ANALYSIS, execution=e.EXECUTION,
                    cells=cells, blocks=e.schedule([cell['id'] for cell in cells]), prepared_wall_ns=1)

    def test_fixed_cases_seeds_and_complete_abba_blocks(self):
        p = self.plan(); e.validate_contract(p)
        self.assertEqual(e.CASES[:8], prior.CASES)
        self.assertEqual(e.LIMITS['seed'], 2026101743)
        self.assertEqual(e.LIMITS['data_seed'], 2026101744)
        self.assertEqual([(v['workers'], v['firms'], v['periods']) for v in e.CASES[-2:]],
                         [(40000, 1500, 15), (12000, 1000, 40)])
        self.assertEqual(e.ARMS, ['baseline', 'cells_e_final'])
        self.assertEqual([e.EXECUTION[k] for k in ['warmup_calls', 'scientific_calls', 'scored_calls']], [40, 80, 960])
        self.assertEqual(len(run.measurement_order(p)), 1000)
        self.assertEqual(len(run.science_order(p)), 80)
        for cell in p['cells']:
            orders = [v['arms'] for block in p['blocks'] for v in block if v['cell'] == cell['id']]
            self.assertEqual(orders.count(['baseline', 'cells_e_final', 'cells_e_final', 'baseline']), 6)
            self.assertEqual(orders.count(['cells_e_final', 'baseline', 'baseline', 'cells_e_final']), 6)
        self.assertEqual(e.ANALYSIS['upper_median_order_statistic'], 10)
        self.assertLess(sum(math.comb(12, k) for k in range(10, 13)) / 2**12, .025)
        for mutation in ['caps', 'order', 'case', 'arm', 'freeze']:
            changed = copy.deepcopy(p)
            if mutation == 'caps': changed['oracle_caps']['MAX_SCHUR_WORK'] += 1
            elif mutation == 'order': changed['blocks'][0].reverse()
            elif mutation == 'case': changed['cases'][-1]['periods'] = 39
            elif mutation == 'arm': changed['arms'][1] = 'cells_e'
            else: changed['prepared_wall_ns'] = 0
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): e.validate_contract(changed)

    def test_every_resource_cap_is_checked_without_estimation(self):
        np = c.np
        y, x = np.ones(8), np.ones((8, 2))
        factors = [np.repeat(np.arange(4), 2), np.tile([0, 1], 4)]
        got = e.oracle_capacity(y, x, factors)
        self.assertEqual(got['schur_product_work'], 16)
        self.assertEqual(got['retained_dimension'], 2)
        for key, cap in [('MAX_ROWS', 7), ('MAX_X', 1), ('MAX_FE', 1), ('MAX_RETAINED', 1),
                         ('MAX_BYTES', got['modeled_peak_bytes'] - 1), ('MAX_SCHUR_WORK', 15), ('MAX_WEIGHT_RATIO', .5)]:
            with self.subTest(cap=key), patch.dict(e.ORACLE_CAPS, {key: cap}), \
                 patch.object(c, 'native', side_effect=AssertionError('candidate must not run')):
                with self.assertRaises(ValueError): e.oracle_capacity(y, x, factors)

    def test_new_shapes_fit_unchanged_analytical_bounds(self):
        for spec in e.CASES[-2:]:
            n, ne, nr, k, q = spec['workers'] * spec['periods'], spec['workers'], spec['firms'], 2, 2
            self.assertLessEqual(n, e.ORACLE_CAPS['MAX_ROWS'])
            self.assertLessEqual(nr, e.ORACLE_CAPS['MAX_RETAINED'])
            self.assertLessEqual(ne + nr, 50000)
            self.assertLessEqual(n * (128 + 256*k + 96*q) + 96*nr*nr + 64*ne*(k+q+2), e.ORACLE_CAPS['MAX_BYTES'])
            self.assertLessEqual(ne * 100, e.ORACLE_CAPS['MAX_SCHUR_WORK'])

    def test_driver_binding_and_full_twenty_setting_order_statistic(self):
        p = self.plan()
        with tempfile.TemporaryDirectory() as directory:
            driver = run.freeze(c.routing.identity(p), Path(directory) / 'driver.json', discovery_unopened=True, profile='engine')
            run.validate_driver(p, driver)
            changed = copy.deepcopy(driver); changed['sources']['bench/tests/test_engine_cells.py'] = 'changed'
            with self.assertRaises(ValueError): run.validate_driver(p, changed)
            changed = copy.deepcopy(driver); changed['plan_sha256'] = '0' * 64
            with self.assertRaises(ValueError): run.validate_driver(p, changed)
        rows = [dict(label=label, cell=cid, arm=arm, phase=phase, block=block, position=position,
                     seconds=.9 if arm == 'cells_e_final' else 1., status='passed')
                for label, cid, arm, phase, block, position in run.measurement_order(p)]
        self.assertEqual(run.summarize(p, rows)['status'], 'passed')
        self.assertEqual(len(run.summarize(p, rows)['settings']), 20)
        for row in rows:
            if row['cell'] == 'weak480k-t4' and row['arm'] == 'cells_e_final' and row['phase'] == 'measure' and row['block'] >= 9:
                row['seconds'] = 1.1
        self.assertEqual(run.summarize(p, rows)['status'], 'failed')
        rows[0], rows[1] = rows[1], rows[0]
        with self.assertRaises(ValueError): run.summarize(p, rows)

    def test_c4_admission_recomputes_d4_and_enforces_both_freezes(self):
        p = self.plan(); p['discovery_plan'] = {'path': 'discovery'}
        discovery = {'kind': c.ENGINE_KIND}
        smoke = dict(kind=c.ENGINE_KIND + '_science', status='passed', smoke=True,
                     plan_sha256=c.routing.identity(discovery), job_id='1', host='science', slots=4,
                     records=[{}], files=[])
        science = dict(job_id='1', host='science', slots=4)
        measurement = {'host': {'hostname': 'd4-host'}}
        values = dict(discovery=discovery, smoke=smoke, science=science, measurement=measurement, isolation_review={})
        evidence = {name: {'path': name} for name in ['smoke', 'science', 'measurement', 'science_accounting',
                                                     'measurement_accounting', 'isolation_review']}
        for mode in ['eligible', 'ineligible', 'wrong-finalist', 'late-plan', 'late-driver']:
            current = copy.deepcopy(p)
            driver = {'frozen_wall_ns': 2}
            if mode == 'late-plan': current['prepared_wall_ns'] = 10
            if mode == 'late-driver': driver['frozen_wall_ns'] = 10
            gate = {'status': 'ineligible' if mode == 'ineligible' else 'eligible',
                    'finalist': 'cells_e' if mode == 'wrong-finalist' else 'cells_e_final'}
            with self.subTest(mode=mode), patch.object(e, 'validate'), patch.object(run, 'validate_driver'), \
                 patch.object(c, 'checked', side_effect=lambda item: item['path']), \
                 patch.object(run, 'unsealed', side_effect=lambda path: values[path]), \
                 patch.object(run, 'check_record_files'), patch.object(c, 'read', return_value={}), \
                 patch.object(run, 'accounting'), patch.object(run, 'isolation_review'), \
                 patch.object(run, 'wall_window', return_value=(10, 20)), \
                 patch.object(c, 'engine_gate', return_value=gate) as recompute:
                if mode == 'eligible':
                    admitted = run.admission(current, driver, evidence)
                    self.assertEqual(admitted['status'], 'admitted')
                    self.assertEqual(admitted['discovery_host'], 'd4-host')
                else:
                    with self.assertRaises(ValueError): run.admission(current, driver, evidence)
                recompute.assert_called_once_with(discovery, science, measurement)

    def test_distinct_host_and_admission_fail_before_candidate_calls(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(run, 'validate_admission'), \
             patch.object(run, 'validate_science'), patch.object(run, 'check_library_limits'), \
             patch.object(c.scc.discovery, 'host', return_value={'slots': 28, 'hostname': 'd4-host.scc.bu.edu'}), \
             patch.object(c, 'native') as native:
            with self.assertRaisesRegex(ValueError, 'distinct physical host'):
                run.measure(self.plan(), {}, {'discovery_host': 'd4-host'}, {}, Path(directory) / 'out')
            native.assert_not_called()
        with patch.object(run, 'validate_admission', side_effect=ValueError('D4 ineligible')), \
             patch.object(c, 'native') as native:
            with self.assertRaisesRegex(ValueError, 'D4 ineligible'): run.science(self.plan(), {}, {}, Path('/unused'))
            native.assert_not_called()

    def test_failing_capacity_stops_preparation_without_candidate(self):
        p = {'kind': c.ENGINE_KIND, 'native_affinity_cores': 4}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for key in ['generator', 'harness', 'plugin']:
                file = root / key; file.write_text(key); p[key] = c.ref(file)
            discovery = root / 'discovery.json'; c.save(discovery, c.routing.seal(p))
            with patch.object(c, 'validate'), patch.object(e, 'tool_files', return_value=[]), \
                 patch.object(e, 'columns', side_effect=ValueError('oracle cap failed')) as columns, \
                 patch.object(c, 'native') as native, patch.object(c.routing_oracle, 'write_reference') as oracle:
                with self.assertRaisesRegex(ValueError, 'oracle cap failed'): e.prepare(root / 'c4', discovery)
                self.assertEqual(columns.call_count, 1)
                native.assert_not_called(); oracle.assert_not_called()
                self.assertFalse((root / 'c4/plan.json').exists())


if __name__ == '__main__': unittest.main()
