import sys
from pathlib import Path
import unittest
import json

try:
    import numpy
    import scipy
except ImportError as error:
    raise unittest.SkipTest('Optimum adjudication needs NumPy/SciPy; skip is not admission') from error
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from ppml_auto_science import compare_optimum_intervals


class OptimumAdjudicationTests(unittest.TestCase):

    def compare(self, candidate, reference, intervals):
        return compare_optimum_intervals(candidate, reference, intervals, rtol='1e-6', atol='1e-10')

    def test_proven_reference_error_and_deterministic_reporting(self):
        args = ([1e-5], ['0.00001000030000000000'], [['0.000009999999999999', '0.000010000000000001']])
        result = self.compare(*args)
        self.assertTrue(result['candidate_inside_optimum_envelope'])
        self.assertTrue(result['reference_accuracy_exception_supported'])
        self.assertEqual(result['unresolved_coordinates'], [])
        self.assertFalse(result['campaign_admission'])
        self.assertEqual(result, self.compare(*args))

    def test_wrong_candidate_cannot_be_excused_by_wrong_reference(self):
        result = self.compare([2e-5], ['0.00003000000000000000'], [['0.000009999999999999', '0.000010000000000001']])
        self.assertFalse(result['candidate_inside_optimum_envelope'])
        self.assertFalse(result['reference_accuracy_exception_supported'])
        self.assertEqual(result['unresolved_coordinates'], [1])

    def test_wide_optimum_and_reference_printing_fail_closed(self):
        for reference, interval in [('0.00001', ['0.000009999999999999', '0.000010000000000001']),
                                    ('0.00001000030000000000', ['0.000009', '0.000011'])]:
            result = self.compare([1e-5], [reference], [interval])
            self.assertFalse(result['reference_accuracy_exception_supported'])
            self.assertEqual(result['unresolved_coordinates'], [1])

    def test_ordinary_agreement_needs_no_exception(self):
        result = self.compare([1e-5], ['0.00001000000000000000'], [['0.000009999999999999', '0.000010000000000001']])
        self.assertEqual(result['unresolved_coordinates'], [])
        self.assertFalse(result['reference_accuracy_exception_supported'])

    def test_pairwise_disagreement_without_proven_reference_error_is_unresolved(self):
        result = self.compare([0.00001000009], ['0.00000999991000000000'],
                              [['0.000010000000000000', '0.000010000000000000']])
        self.assertTrue(result['candidate_inside_optimum_envelope'])
        self.assertFalse(result['coordinates'][0]['reference_outside_optimum_envelope'])
        self.assertFalse(result['reference_accuracy_exception_supported'])
        self.assertEqual(result['unresolved_coordinates'], [1])

    def test_one_proven_error_cannot_mask_another_unresolved_coordinate(self):
        result = self.compare([1e-5, 1e-5], ['0.00001000030000000000', '0.00001'],
            [['0.000009999999999999', '0.000010000000000001']]*2)
        self.assertTrue(result['coordinates'][0]['reference_outside_optimum_envelope'])
        self.assertEqual(result['unresolved_coordinates'], [2])
        self.assertFalse(result['reference_accuracy_exception_supported'])

    def test_invalid_intervals_rejected(self):
        for interval in (['NaN', '1'], ['1', '0'], ['0']):
            with self.assertRaises(ValueError):
                self.compare([1e-5], ['0.00001000000000000000'], [interval])


if __name__ == '__main__':
    unittest.main()
