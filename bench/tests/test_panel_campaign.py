"""Checks for real-panel preparation, FE comparison and scientific rejection."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import panel_campaign as panel
from campaign import make_dataset
from campaign_oracle import oracle


class PanelTests(unittest.TestCase):
    def test_prior_input_audit_rejects_changed_rows_or_response(self):
        frame = pd.DataFrame(dict(obs_id=[1, 2, 3], g1=[1, 1, 2], g2=[1, 1, 1],
                                  x1=[0., 1., 1.], y=[2., 3., 4.]))
        prior = pd.DataFrame(dict(observation_key=[1, 2, 3], worker=[1, 1, 2], firm=[1, 1, 1],
                                  y_adjusted=[2., 2.5, 3.5]))
        result = panel.audit_prior(frame, prior)
        self.assertLess(result['adjustment_max_error'], 1e-14)
        prior.loc[0, 'worker'] = 3
        with self.assertRaisesRegex(ValueError, 'identities differ'):
            panel.audit_prior(frame, prior)
        prior.loc[0, 'worker'] = 1
        prior.loc[0, 'y_adjusted'] = 2.1
        with self.assertRaisesRegex(ValueError, 'control adjustment'):
            panel.audit_prior(frame, prior)

    def test_complete_private_preparation_with_small_inputs(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            specifications = {}
            for name, years in [('long', list(range(1996, 2002))), ('short', [1999, 2001])]:
                worker = np.repeat(np.arange(1, 61), len(years))
                year = np.tile(years, 60)
                firm = (worker+year) % 3+1
                rng = np.random.default_rng(19)
                frame = pd.DataFrame(dict(worker=worker, firm=firm, year=year,
                                          observation_key=np.arange(1, len(worker)+1),
                                          age=year-np.repeat(np.arange(1950, 2010), len(years)),
                                          y=rng.normal(size=len(worker))))
                frame.to_csv(root/(name+'.csv'), index=False)
                specifications[name] = dict(panel.PANELS[name], rows=len(frame), workers=60, firms=3)
            with patch.dict(panel.PANELS, specifications):
                panel.prepare(root, root/'long.csv', root/'short.csv')
            self.assertTrue((root/'manifest.json').exists())
            self.assertTrue((root/'data/warm.dta').exists())
            self.assertTrue((root/'smoke-data/warm.dta').exists())

    def test_manifest_ladder_and_license(self):
        frame = pd.DataFrame(dict(x1=[1.], y=[2.]))
        cells = [panel.cell_for(p, frame, t) for p in panel.PANELS for t in panel.THREADS]
        self.assertEqual(panel.THREADS, [1, 2, 4, 8, 16])
        self.assertEqual(sum(len(c['arms']) for c in cells)*panel.REPETITIONS, 108)
        self.assertTrue(all(('reghdfe' in c['arms']) == (c['threads'] <= 4) for c in cells))

    def test_variable_control_count_dense_witness(self):
        cell = dict(model='ols', design='akm-strong', rows=150, regressors=1,
                    effects=2, vce='iid', weights='unit')
        with tempfile.TemporaryDirectory() as d:
            frame, _ = make_dataset(cell, Path(d)/'test.dta')
            expected = oracle(frame, cell)
        self.assertEqual(len(expected['beta']), 1)
        self.assertLess(expected['residual_score'], 1e-12)

    def test_preparation_rejects_different_sample(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d)/'input.csv'
            pd.DataFrame(dict(year=[1999, 2001], worker=[1, 1], firm=[1, 1],
                              observation_key=[1, 2], y=[1., 2.])).to_csv(path, index=False)
            with self.assertRaisesRegex(ValueError, 'established fevc sample'):
                panel.prepare_panel(path, 'short')

    def test_connectivity_audit_rejects_disconnected_and_singleton(self):
        with tempfile.TemporaryDirectory() as d:
            frame = pd.DataFrame(dict(obs_id=[1, 2, 3, 4], g1=[1, 1, 2, 2],
                                      g2=[1, 1, 2, 2], y=[1., 2., 3., 4.], x1=[0, 1, 0, 1]))
            with self.assertRaisesRegex(ValueError, 'connected retained panel'):
                panel.save_dataset(frame, Path(d)/'disconnected.dta')
            frame.g2 = [1, 1, 1, 2]
            with self.assertRaisesRegex(ValueError, 'singleton'):
                panel.save_dataset(frame, Path(d)/'singleton.dta')

    def test_comparison_rejects_a_changed_saved_effect(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            records = []
            for name in ['a', 'b']:
                directory = root/name
                directory.mkdir()
                np.savez_compressed(directory/'fitted.npz', fitted=[1., 2.])
                np.savez_compressed(directory/'effects.npz', saved1=[-.5, .5], saved2=[0.])
                capture = dict(beta=[1.], covariance=[[1.]], N=2, sample_sha256='same', omitted=[],
                               finite_slope_rank=1, singleton_deletions=0, separation_deletions=0,
                               clusters=None, absorbed_dof=1, covariance_policy='stock-ols-iid',
                               residual_dof=1, correction_dof=1, maximum_model_score=0.)
                records.append(dict(status='ok', directory=str(directory), capture=capture,
                                    metadata=dict(rss='1')))
            cell = dict(model='ols', vce='iid', save_fe=True)
            self.assertTrue(panel.compare(*records, cell)['eligible'])
            np.savez_compressed(root/'b/effects.npz', saved1=[-.6, .6], saved2=[0.])
            self.assertIn('centered saved1', panel.compare(*records, cell)['reasons'])


if __name__ == '__main__':
    unittest.main()
