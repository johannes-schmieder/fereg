import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from campaign_export import harness_test_summary


class HarnessSummaryTests(unittest.TestCase):
    def test_buffered_stdout_after_unittest_result_is_not_a_failure(self):
        result = harness_test_summary('Ran 454 tests in 100.755s\n\nOK\nreference candidate ratio\n')
        self.assertEqual(result, {'count': 454, 'passed': True})

    def test_failed_and_missing_results_are_not_successes(self):
        for text in ['Ran 2 tests in 1s\nFAILED (failures=1)\n',
                     'Ran 2 tests in 1s\nOK\nFAILED (errors=1)\n', 'Ran 2 tests in 1s\n']:
            self.assertFalse(harness_test_summary(text)['passed'])
        with self.assertRaises(ValueError):
            harness_test_summary('OK\n')
