"""Independent identities for post-campaign output diagnostics."""
from pathlib import Path
import sys
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from panel_accuracy import centered_levels, differences, model_scores, selected_positions


class AccuracyTests(unittest.TestCase):
    def test_all_positions_keep_skips_and_require_exact_native_reference(self):
        records = [dict(cell='cell', arm=arm, repetition=rep,
                        status='ok' if arm == 'fereg' or rep == 1 else 'skipped-after-failure',
                        capture=dict(hash='same'), metadata=dict(rss='2'))
                   for arm in ['fereg', 'fixest'] for rep in [1, 2, 3]]
        campaign = dict(complete=True, native_gate_pass=True, records=records)
        selected = selected_positions(campaign, 'cell', ['fereg', 'fixest'], 3)
        self.assertEqual(len(selected), 6)
        self.assertEqual(sum(r['status'] == 'skipped-after-failure' for r in selected), 2)
        records[1]['capture']['hash'] = 'different'
        with self.assertRaisesRegex(ValueError, 'exact repeat'):
            selected_positions(campaign, 'cell', ['fereg', 'fixest'], 3)

    def test_centering_removes_only_constant_gauge(self):
        codes = np.array([0, 0, 1])
        first, counts = centered_levels(np.array([1., 1., 4.]), codes)
        shifted, _ = centered_levels(np.array([8., 8., 11.]), codes)
        np.testing.assert_array_equal(first, shifted)
        changed, _ = centered_levels(np.array([1., 1., 7.]), codes)
        result = differences(first, changed, rtol=1e-7, atol=1e-9, weights=counts)
        self.assertAlmostEqual(result['maximum_absolute_error'], 2.)
        self.assertAlmostEqual(result['rmse'], np.sqrt(2.5))
        self.assertAlmostEqual(result['observation_weighted_rmse'], np.sqrt(2.))

    def test_independent_score_sees_firm_errors_with_zero_worker_scores(self):
        y = np.ones(4)
        fitted = y-np.array([1., -1., 1., -1.])*.001
        codes = [np.array([0, 0, 1, 1]), np.array([0, 1, 0, 1])]
        scores = model_scores(y, fitted, codes, np.ones((4, 1)))
        self.assertAlmostEqual(scores['worker'], 0.)
        self.assertAlmostEqual(scores['controls'], 0.)
        self.assertGreater(scores['firm'], 4e-4)
        self.assertEqual(scores['maximum'], scores['firm'])


if __name__ == '__main__':
    unittest.main()
