"""D5 is a fresh four-arm ownership study; all native execution is mocked."""
import copy
from collections import Counter
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compact_cells as c
from test_compact_cells import case_bytes


class SOwnedProtocol(unittest.TestCase):
    def plan(self):
        cells = [dict(id=f'{family}-t{worker}', family=family, threads=worker,
                      inputs={arm: {} for arm in [*c.S_OWNED_CONTROLS, 'cells_s_owned']})
                 for family in c.FAMILIES for worker in [1, 4]]
        return dict(kind=c.S_OWNED_KIND, arms=c.S_OWNED_ARMS, limits=c.S_OWNED_LIMITS,
                    families=c.FAMILIES, library_limits=c.LIBRARY_LIMITS, cells=cells,
                    blocks=c.schedule([cell['id'] for cell in cells], arms=c.S_OWNED_ARMS, limits=c.S_OWNED_LIMITS))

    def evidence(self):
        plan = self.plan()
        rows = [dict(label=label, cell=cid, arm=arm, phase=phase, block=block, repeat=repeat,
                     status='passed', seconds={'baseline': 1., 'fit_owned': 1., 'cells_s': .9, 'cells_s_owned': .8}[arm],
                     science_sha256=cid + ('auto' if arm in ['baseline', 'fit_owned'] else 's'))
                for label, cid, arm, phase, block, repeat in c.engine_measurement_order(plan)]
        science = {}
        measurement = dict(kind=c.S_OWNED_KIND + '_measurement', status='complete',
                           plan_sha256=c.routing.identity(plan), science_sha256=c.routing.identity(science),
                           records=rows, files=[])
        return plan, science, measurement

    def gate(self, evidence=None):
        with patch.object(c, 'validate'), patch.object(c, 'validate_science'), \
             patch.object(c, 'engine_gate', side_effect=AssertionError('D4 gate is not a D5 prerequisite')):
            return c.s_owned_gate(*(evidence or self.evidence()))

    def test_frozen_counts_balanced_schedule_and_thirteen_controls(self):
        plan = self.plan(); c.validate_contract(plan)
        self.assertEqual(c.contract(plan), (c.S_OWNED_ARMS, c.S_OWNED_LIMITS))
        self.assertEqual(c.S_OWNED_ARMS, ['baseline', 'cells_s', 'fit_owned', 'cells_s_owned'])
        self.assertEqual(c.S_OWNED_LIMITS['seed'], 2026101752)
        self.assertEqual(len(set(c.control_arms(plan))), 13)
        self.assertEqual(set(c.control_arms(plan)), set(c.ENGINE_CONTROLS) | set(c.ENGINE_ARMS))
        order = c.engine_measurement_order(plan)
        self.assertEqual(Counter(row[3] for row in order), {'warmup': 96, 'measure': 1152, 'diagnostic': 288})
        self.assertEqual(24 * len(c.S_OWNED_ARMS) * 2, 192)
        self.assertEqual(24 * len(c.S_OWNED_CONTROLS) * 2, 624)
        self.assertEqual(len(c.S_OWNED_ARMS) * 2 + len(c.S_OWNED_CONTROLS) * 2, 34)
        for cell in plan['cells']:
            positions = [slot['arm'] for block in plan['blocks'] for slot in block if slot['cell'] == cell['id']]
            for position in range(4):
                self.assertEqual(Counter(positions[position::4]), {arm: 3 for arm in c.S_OWNED_ARMS})
        for field in ['seed', 'blocks', 'diagnostic_repeats']:
            changed = copy.deepcopy(plan); changed['limits'][field] += 1
            with self.subTest(field=field), self.assertRaises(ValueError): c.validate_contract(changed)

    def test_only_fixed_finalist_and_complete_solve_gates(self):
        got = self.gate()
        self.assertEqual(got['status'], 'eligible')
        self.assertEqual(got['finalist'], 'cells_s_owned')
        self.assertNotIn('first_solver_comparisons', got)
        for reference, gains in [('baseline', 17), ('cells_s', 1)]:
            plan, science, measurement = self.evidence()
            exempt = {cell['id'] for cell in plan['cells'][:gains]}
            for row in measurement['records']:
                if row['arm'] == 'cells_s_owned' and row['cell'] not in exempt:
                    row['seconds'] = 1. if reference == 'baseline' else .9
            self.assertEqual(self.gate((plan, science, measurement))['status'], 'ineligible')
        for arm in ['fit_owned', 'cells_s_owned']:
            plan, science, measurement = self.evidence()
            for row in measurement['records']:
                if row['arm'] == arm and row['cell'] == 'weak-t1': row['seconds'] = 1.06
            self.assertEqual(self.gate((plan, science, measurement))['status'], 'ineligible')

    def test_incremental_gains_must_cover_two_distinct_families(self):
        for chosen, expected in [({'weak-t1', 'weak-t4'}, 'ineligible'),
                                 ({'weak-t1', 'dense-t1'}, 'eligible')]:
            plan, science, measurement = self.evidence()
            for row in measurement['records']:
                if row['arm'] == 'cells_s_owned': row['seconds'] = .8 if row['cell'] in chosen else .9
            got = self.gate((plan, science, measurement))
            self.assertEqual(got['status'], expected)
            self.assertEqual(got['counts']['finalist_vs_s']['material_gains'], 2)

    def test_both_exact_pairs_are_enforced_in_every_measurement_phase(self):
        for arm in ['fit_owned', 'cells_s_owned']:
            for phase in ['warmup', 'measure', 'diagnostic', 'all']:
                plan, science, measurement = self.evidence()
                for row in measurement['records']:
                    if row['arm'] == arm and row['cell'] == 'weak-t1' and (phase == 'all' or row['phase'] == phase):
                        row['science_sha256'] = 'changed'
                with self.subTest(arm=arm, phase=phase), self.assertRaises(ValueError):
                    self.gate((plan, science, measurement))

    def test_missing_reordered_invalid_or_rebound_measurement_fails_closed(self):
        for mutation in ['missing', 'order', 'duplicate', 'nan', 'zero', 'failed', 'plan']:
            plan, science, measurement = self.evidence()
            rows = measurement['records']
            if mutation == 'missing': rows.pop()
            elif mutation == 'order': rows[0], rows[1] = rows[1], rows[0]
            elif mutation == 'duplicate': rows[-1] = copy.deepcopy(rows[-2])
            elif mutation == 'nan': rows[-1]['seconds'] = float('nan')
            elif mutation == 'zero': rows[-1]['seconds'] = 0.
            elif mutation == 'failed': rows[-1]['status'] = 'failed'
            else: measurement['plan_sha256'] = 'changed'
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): self.gate((plan, science, measurement))

    def test_new_request_and_prior_thirteen_arm_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.write_bytes(case_bytes([b'estimate-ols', b'threads=1', b'solver=auto', b'preconditioner=auto']))
            inputs = {}
            for arm in [*c.S_OWNED_CONTROLS, 'cells_s_owned']:
                target = root / arm
                c.routing.prepare(source, target, arm, 1)
                c.validate_engine_request(target, arm, 1)
                inputs[arm] = c.ref(target)
            with self.assertRaises(ValueError): c.validate_engine_request(root / 'cells_s_owned', 'cells_s', 1)
            harness, plugin = root / 'harness', root / 'd4-plugin'
            harness.write_text('harness'); plugin.write_text('frozen-D4')
            old = dict(id='weak-t1', family='weak', threads=1, inputs={arm: inputs[arm] for arm in c.ARMS})
            parent = root / 'parent.json'; c.save(parent, c.routing.seal({'cells': [old]}))
            prior = dict(kind=c.ENGINE_KIND, harness=c.ref(harness), plugin=c.ref(plugin),
                         cells=[{**old, 'inputs': {arm: inputs[arm] for arm in c.S_OWNED_CONTROLS}}])
            prior['frozen_files'] = {item['path']: item['sha256'] for item in
                                     [prior['harness'], prior['plugin'], *prior['cells'][0]['inputs'].values()]}
            previous = root / 'prior.json'; c.save(previous, c.routing.seal(prior))
            plan = dict(kind=c.S_OWNED_KIND, harness=prior['harness'], parent_plan=c.ref(parent), prior_engine_plan=c.ref(previous),
                        cells=[{**old, 'inputs': inputs}])
            with patch.object(c, 'validate_contract'), patch.object(c, 'engine_gate', side_effect=AssertionError):
                bindings = c.s_owned_control_bindings(plan)
                self.assertEqual(bindings[str(plugin.resolve())], c.digest(plugin))
                for arm in c.S_OWNED_CONTROLS:
                    self.assertEqual(c.control_parent(plan, {}, arm)['plugin'], prior['plugin'])
                for change in ['extra-option', 'body']:
                    changed = copy.deepcopy(plan)
                    target = root / change
                    data = (case_bytes([b'estimate-ols', b'threads=1', b'solver=auto', b'preconditioner=auto',
                                        b'inject=routing_cells_s', b'tolerance=1e-4']) if change == 'extra-option'
                            else (root / 'cells_s').read_bytes().replace(b'body\n', b'Body\n'))
                    target.write_bytes(data)
                    c.validate_engine_request(target, 'cells_s', 1)
                    changed['cells'][0]['inputs']['cells_s'] = c.ref(target)
                    with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'frozen D4 bytes'):
                        c.s_owned_control_bindings(changed)
                plugin.write_text('rebound')
                with self.assertRaisesRegex(ValueError, 'frozen file changed'): c.s_owned_control_bindings(plan)

    def test_smoke_requires_the_full_first_setting_science_and_thirteen_exact_controls(self):
        plan = self.plan(); plan.update(parent_plan={'path': 'parent'}, plugin='candidate')
        cell = plan['cells'][0]
        fingerprint = lambda arm: c.routing.identity('auto' if arm in ['baseline', 'fit_owned'] else
                                                     's' if arm in ['cells_s', 'cells_s_owned'] else arm)
        records = [dict(cell=cell['id'], arm=arm, repeat=repeat, label=f"{cell['id']}-{arm}-{repeat}",
                        status='executed', resources={}, oracle_status='passed', model_discrete_sha256='0' * 64,
                        science_sha256=fingerprint(arm)) for arm in c.S_OWNED_ARMS for repeat in range(2)]
        controls = [dict(cell=cell['id'], arm=arm, status='passed', **{
            role: dict(cell=cell['id'], arm=arm, status='passed', phase=role + '_control',
                       plugin=role, science_sha256=fingerprint(arm)) for role in ['parent', 'candidate']})
                    for arm in c.S_OWNED_CONTROLS]
        binding = {'plan': {'path': 'c5'}, 'driver': {'path': 'driver'}}
        receipt = dict(kind=c.S_OWNED_KIND + '_science', status='passed', smoke=True,
                       plan_sha256=c.routing.identity(plan), records=records,
                       fingerprints={cell['id'] + '/' + arm: fingerprint(arm) for arm in c.S_OWNED_ARMS},
                       parent_controls=controls, parent_control_status='passed', parent_control_count=13,
                       parent_control_native_calls=26, confirmation=binding, files=[])
        with patch.object(c, 'engine_confirmation_bindings', return_value=binding), \
             patch.object(c, 'control_parent', return_value={'plugin': 'parent'}), \
             patch.object(c, 'read', return_value=c.routing.seal({})):
            c.validate_science(plan, receipt, smoke=True)
            with self.assertRaises(ValueError): c.validate_science(plan, receipt)
            for mutation in ['missing-science', 'science-order', 'oracle', 'resources', 'discrete', 'ownership',
                             'missing-control', 'control-order', 'control-calls', 'control-science', 'control-plugin']:
                changed = copy.deepcopy(receipt)
                if mutation == 'missing-science': changed['records'].pop()
                elif mutation == 'science-order': changed['records'][0], changed['records'][1] = changed['records'][1], changed['records'][0]
                elif mutation == 'oracle': changed['records'][0]['oracle_status'] = 'failed'
                elif mutation == 'resources': del changed['records'][0]['resources']
                elif mutation == 'discrete': changed['records'][0]['model_discrete_sha256'] = '1' * 64
                elif mutation == 'ownership':
                    for row in changed['records']:
                        if row['arm'] == 'cells_s_owned': row['science_sha256'] = '1' * 64
                    changed['fingerprints'][cell['id'] + '/cells_s_owned'] = '1' * 64
                elif mutation == 'missing-control': changed['parent_controls'].pop()
                elif mutation == 'control-order': changed['parent_controls'].reverse()
                elif mutation == 'control-calls': changed['parent_control_native_calls'] = 25
                elif mutation == 'control-science': changed['parent_controls'][0]['parent']['science_sha256'] = '1' * 64
                else: changed['parent_controls'][0]['parent']['plugin'] = 'other'
                with self.subTest(mutation=mutation), self.assertRaises(ValueError): c.validate_science(plan, changed, smoke=True)

    def test_c5_freeze_required_before_any_d5_call(self):
        with patch.object(c, 'validate'), patch.object(c, 'native') as native:
            with self.assertRaisesRegex(ValueError, 'frozen C5'):
                c.science(self.plan(), Path('/unused'), smoke=True)
            native.assert_not_called()


if __name__ == '__main__': unittest.main()
