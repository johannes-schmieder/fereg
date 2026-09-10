"""Fail-closed guards for the separate ADR 0030 development comparison."""
import copy
import unittest
from unittest.mock import patch
from scripts import ols_wide_cholesky_development as wide


def fit():
    measures = [0.0] * 32
    for i in (0, 1, 2, 23, 31): measures[i] = 1.0
    measures[3] = 1e-8
    measures[5] = 1.01
    measures[13] = measures[20] = 1e-5
    return {'scientific': {
        'stata_rc': 0, 'sample_identity_sha256': 'sample', 'singleton_identity_sha256': 'singletons',
        'coefficient_names': ['x1'], 'coefficient_status': [0], 'vce_shape': [1, 1],
        'dof': {'fereg_dof': [100, 1]}, 'outputs': {},
        'coefficients': [0.2], 'vce': [1e-5], 'fit': {'fereg_fit': [0.9]}, 'numeric_hex_sha256': 'numeric',
        'certificates': {'converged': True, 'values': {'normal_equation_linf': 1e-15, 'fe_projection_linf': 1e-16,
            'vce_symmetry_linf': 0.0, 'reconstruction_linf': 1e-15},
            'digests': {'fereg_certificate': 'certificate', 'solver_contract': 'solver', 'fereg_status': 'status'}},
        'solver': {'source_columns': [1,2,3,4,5,6,7,8,11,12,13,14,15,16], 'values': [1,2,1,2,1e-12,1e-16,1e-16,1e-16,0,0,0,0,0,0]}},
        'raw': {'rows': 32, 'columns': 2, 'values': [v for m in measures for v in (1.0, m)]}}


class WideComparisonTests(unittest.TestCase):
    def compare(self, a, b, **kwargs):
        with patch.object(wide.dev.capture, '_matrix', side_effect=lambda raw, *_: raw), patch.object(wide.dev.capture, '_finite_matrix', side_effect=lambda raw: raw['values']):
            return wide.compare_science(a, b, **kwargs)

    def test_rounding_and_condition_proxy_are_explicit_exceptions(self):
        a = fit(); b = copy.deepcopy(a)
        b['scientific']['coefficients'][0] += 1e-14
        b['scientific']['numeric_hex_sha256'] = 'new'
        b['scientific']['certificates']['digests']['fereg_certificate'] = 'new'
        b['raw']['values'][11] = 1.02
        b['raw']['values'][61] = 1e-257
        self.assertFalse(self.compare(a, b)['exact'])
        with self.assertRaises(ValueError): self.compare(a, b, unchanged=True)

    def test_registered_numerical_envelopes_remain_binding(self):
        for field in ('coefficients', 'vce'):
            a = fit(); b = copy.deepcopy(a); b['scientific'][field][0] += 0.01
            with self.assertRaises(ValueError): self.compare(a, b)

    def test_sample_status_dof_and_output_are_exact(self):
        for key, value in [('sample_identity_sha256', 'wrong'), ('coefficient_status', [1]), ('dof', {}), ('outputs', {'saved': 'changed'})]:
            a = fit(); b = copy.deepcopy(a); b['scientific'][key] = value
            with self.assertRaises(ValueError): self.compare(a, b)

    def test_bad_absolute_certificate_and_incomplete_raw_fail(self):
        for index, value in [(1, 0.0), (13, 1e-8), (9, 1e-4), (11, float('nan'))]:
            a = fit(); b = copy.deepcopy(a); b['raw']['values'][index] = value
            with self.assertRaises(ValueError): self.compare(a, b)
        a = fit(); b = copy.deepcopy(a); b['raw']['rows'] = 31
        with self.assertRaises(ValueError): self.compare(a, b)

    def test_retry_and_rank_policy_cannot_change(self):
        a = fit(); b = copy.deepcopy(a); b['scientific']['solver']['values'][8] = 1
        with self.assertRaises(ValueError): self.compare(a, b)
        b = copy.deepcopy(a); b['raw']['values'][7] = 2e-8
        with self.assertRaises(ValueError): self.compare(a, b)

    def test_pilot_measurement_may_change_but_operator_iteration_count_cannot(self):
        a = fit(); b = copy.deepcopy(a)
        b['scientific']['solver']['values'][13] = 0.2
        self.assertEqual(self.compare(a, b)['pilot_contractions'], [[0], [0.2]])
        b['scientific']['solver']['values'][12] = 1
        with self.assertRaises(ValueError): self.compare(a, b)

    def test_failed_convergence_and_nonfinite_numerics_fail(self):
        a = fit(); b = copy.deepcopy(a); b['scientific']['certificates']['converged'] = False
        with self.assertRaises(ValueError): self.compare(a, b)
        b = copy.deepcopy(a); b['scientific']['vce'][0] = float('nan')
        with self.assertRaises(ValueError): self.compare(a, b)


if __name__ == '__main__': unittest.main()
