import csv
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]/'scripts'))
from ppml_auto_reference import read_reference_rows, compare_reference_fit


class ReferenceRowsTests(unittest.TestCase):
    def row(self):
        return dict(arm='ppmlhdfe', repetition='1', rc='0', N='20', ll='-10',
                    deviance='4', converged='1', rank='1', df_a='3', df_resid='.',
                    b_names='x1 _cons', b_values='0.2 0', v_names='x1 _cons',
                    v_values='0.1 0 0 0', reference_acceleration='conjugate_gradient',
                    reference_transform='symmetric_kaczmarz')

    def read(self, rows):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'reference.tsv'
            with path.open('w') as stream:
                writer = csv.DictWriter(stream, fieldnames=self.row(), delimiter='\t')
                writer.writeheader()
                writer.writerows(rows)
            return read_reference_rows(path, 1)

    def test_complete_record_does_not_admit_campaign(self):
        result = self.read([self.row()])
        self.assertTrue(result['reference_available'])
        self.assertFalse(result['campaign_admission'])

    def test_truncated_covariance_is_unavailable_not_numerical_disagreement(self):
        result = self.read([{**self.row(), 'v_values': '0.1 0 0'}])
        self.assertFalse(result['reference_available'])
        self.assertEqual(len(result['rejected_records']), 1)

    def test_complete_fit_survives_later_incomplete_record(self):
        result = self.read([self.row(), {**self.row(), 'v_values': '0.1'}])
        self.assertEqual(len(result['complete_fits']), 1)
        self.assertEqual(result['rejected_records'][0]['row'], 2)

    def test_nonfinite_wrong_route_and_unsuccessful_records_fail_closed(self):
        for patch in ({'v_values': 'nan 0 0 0'}, {'rc': '430'},
                      {'reference_acceleration': 'lsmr'}, {'b_names': 'z1 _cons'}):
            self.assertFalse(self.read([{**self.row(), **patch}])['reference_available'])

    def test_matrix_stripes_must_describe_the_same_unique_coordinates(self):
        for patch in ({'v_names': 'x1 other'}, {'b_names': 'x1 x1', 'v_names': 'x1 x1'},
                      {'b_names': 'x1', 'b_values': '0.2'}):
            self.assertFalse(self.read([{**self.row(), **patch}])['reference_available'])

    def test_invalid_control_count_fails_before_reading(self):
        for controls in (-1, 1.5, True):
            with self.assertRaises(ValueError):
                read_reference_rows('not-opened.tsv', controls)

    def test_no_worse_objective_requires_independent_poisson_identity(self):
        ref = self.read([self.row()])['complete_fits'][0]['fit']
        candidate = {**ref, 'df_resid': 16, 'll': -9, 'deviance': 2}
        science = dict(coefficient_rtol=1e-7, coefficient_atol=1e-10,
                       vce_rtol=1e-6, vce_atol=1e-10,
                       likelihood_rtol=1e-8, likelihood_atol=1e-10)
        self.assertTrue(compare_reference_fit(candidate, ref, 1, -16, science)['comparison_pass'])
        worse = {**candidate, 'll': -11, 'deviance': 6}
        self.assertFalse(compare_reference_fit(worse, ref, 1, -16, science)['comparison_pass'])
        for patch in ({'ll': float('nan')}, {'deviance': -1}, {'ll': -8}):
            with self.assertRaises(ValueError):
                compare_reference_fit({**candidate, **patch}, ref, 1, -16, science)


if __name__ == '__main__':
    unittest.main()
