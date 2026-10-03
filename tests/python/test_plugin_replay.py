"""Acceptance checks must detect numerical/routing drift and distinct harness errors."""
import copy
import math
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from replay_plugins import allowed_difference, differences, exact_view, expected_for_fixes
from replay_psd import eigensystem


class ReplayChecks(unittest.TestCase):
    def fixture(self):
        return {'return_code': 0, 'columns': [['3ff0000000000000']], 'matrices': {
            'RB': {'bits': ['3ff0000000000000']},
            'RSOLVER': {'columns': 16, 'bits': ['0000000000000000'] * 32},
            'RRUNTIME': {'bits': ['0000000000000000'] * 60},
            'RMEMORY': {'bits': ['0000000000000000']}}}

    def test_one_bit_coefficient_and_route_changes_are_visible(self):
        base = self.fixture()
        for name, index in [('RB', 0), ('RSOLVER', 1), ('RRUNTIME', 1), ('RRUNTIME', 57)]:
            changed = copy.deepcopy(base)
            changed['matrices'][name]['bits'][index] = '3ff0000000000001'
            self.assertTrue(differences(exact_view(base, False), exact_view(changed, False)))

    def test_only_documented_process_fields_are_excluded(self):
        base = self.fixture()
        changed = copy.deepcopy(base)
        for name, index in [('RSOLVER', 8), ('RRUNTIME', 35), ('RMEMORY', 0)]:
            changed['matrices'][name]['bits'][index] = '3ff0000000000001'
        self.assertEqual(differences(exact_view(base, False), exact_view(changed, False)), [])

    def test_return_codes_and_observation_outputs_are_exact(self):
        base = self.fixture()
        changed = copy.deepcopy(base)
        changed['return_code'] = 498
        changed['columns'][0][0] = '0000000000000000'
        self.assertEqual(len(differences(exact_view(base, False), exact_view(changed, False))), 2)

    def test_intentional_changes_are_field_scoped_and_science_is_required(self):
        self.assertTrue(allowed_difference('/matrices/RV/bits/1', ['/matrices/RV/bits']))
        self.assertFalse(allowed_difference('/matrices/RB/bits/1', ['/matrices/RV/bits']))
        self.assertFalse(allowed_difference('/matrices/RV/bits/19', ['/matrices/RV/bits/1']))
        self.assertFalse(allowed_difference('fixed-thread-repeat/matrices/RV/bits/1', ['/matrices/RV/bits']))
        old = {'known_defect': 'slope-dof', 'return_code': 498, 'beta': [1.]}
        repaired = expected_for_fixes(old, {'slope-dof'})
        self.assertEqual(repaired['return_code'], 0)
        self.assertEqual(repaired['beta'], old['beta'])
        self.assertNotIn('known_defect', repaired)
        self.assertEqual(old['return_code'], 498)

    def test_independent_psd_target_has_analytic_spectrum(self):
        values, fixed = eigensystem([[1., 2., 0.], [2., 1., 0.], [0., 0., 4.]])
        for a,b in zip(sorted(values), [-1., 3., 4.]): self.assertAlmostEqual(a,b,places=13)
        target = [[1.5,1.5,0.],[1.5,1.5,0.],[0.,0.,4.]]
        for row, expected in zip(fixed,target):
            for a,b in zip(row,expected): self.assertTrue(math.isclose(a,b,abs_tol=1e-14))


if __name__ == '__main__': unittest.main()
