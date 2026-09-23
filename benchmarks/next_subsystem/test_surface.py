"""Boundary checks for additive generated-term benchmark semantics."""
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

import numpy as np
import pandas as pd

from surface import columns, configure, stage
from surface_audit import absorbed_design


class SurfaceTests(unittest.TestCase):
    def test_keyed_operators_keep_panel_boundaries_and_time_gaps(self):
        frame = pd.DataFrame(dict(ts_panel=[1, 1, 1, 2, 2], ts_time=[1, 2, 4, 1, 2],
                                  x1=[1., 3., 7., 11., 17.],
                                  x2=[2., 5., 9., 13., 19.],
                                  x3=[4., 8., 12., 16., 24.]))
        cell = configure(dict(model='ols', effects=['g1'], k=3,
                              terms='time_series', ts_operators=['L', 'F', 'D']))
        expected = [[np.nan, 5, np.nan], [1, np.nan, 4], [np.nan, np.nan, np.nan],
                    [np.nan, 19, np.nan], [11, np.nan, 8]]
        np.testing.assert_array_equal(columns(frame, cell), expected)
        # Availability filtering must not manufacture a new adjacent time.
        np.testing.assert_array_equal(columns(frame.iloc[[0, 2, 3, 4]], cell)[:, 0],
                                      [np.nan, np.nan, np.nan, 11])

    def test_multiple_slope_partition_span(self):
        frame = pd.DataFrame(dict(g1=[1]*6+[2]*6, g2=[1, 2]*6,
                                  z1=np.arange(12.), z2=np.sin(np.arange(12.))))
        cell = configure(dict(model='ols', effects=['g1', 'g2'], k=2,
                              slope_partitions={'g1': ['z1'], 'g2': ['z2']}))
        F, rank = absorbed_design(frame, cell)
        self.assertEqual(rank, 7)
        np.testing.assert_array_equal(F.toarray()[:, 2],
                                      np.r_[np.arange(6.), np.zeros(6)])
        self.assertEqual(cell['effects_r'], 'g1[z1] + g2[z2]')

    def test_wide_metadata_is_captured_outside_startup_arguments(self):
        root = Path(__file__).resolve().parents[1] / 'local_fixest_gap'
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            for name in ('measure.py', 'stata_worker.do', 'r_worker.R'):
                (folder/name).write_bytes((root/name).read_bytes())
            stage(folder)
            text = (folder/'measure.py').read_text()
            compile(text, 'staged-measure', 'exec')
            self.assertIn('surface-args.do', text)
            self.assertNotIn('"string_columns", "rhs"', text)
            self.assertIn('include `"`output\'/surface-args.do"\'',
                          (folder/'stata_worker.do').read_text())

    def test_oversized_slope_oracle_refuses_before_dense_allocation(self):
        frame = pd.DataFrame(dict(g1=np.arange(601), z1=np.ones(601)))
        cell = dict(effects=['g1'], slope_partitions={'g1': ['z1']})
        with patch('surface_audit.pd.get_dummies', side_effect=AssertionError('allocated')):
            with self.assertRaisesRegex(ValueError, 'bounded explicit width'):
                absorbed_design(frame, cell)


if __name__ == '__main__':
    unittest.main()
