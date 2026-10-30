"""Checks for real-panel preparation, FE comparison and scientific rejection."""
from pathlib import Path
import sys
import json
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

    def test_reuse_keeps_inputs_and_schedules_a_panel_subset(self):
        with tempfile.TemporaryDirectory() as d:
            old, new = Path(d)/'old', Path(d)/'new'
            for folder in ['data', 'smoke-data', 'input']:
                (old/folder).mkdir(parents=True)
            datasets = {}
            for name in panel.PANELS:
                for folder, key in [('data', name), ('smoke-data', 'smoke-'+name)]:
                    path = old/folder/f'ols-veneto-{name}-n10.dta'
                    path.write_bytes((name+folder).encode())
                    datasets[key] = dict(sha256=panel.sha256(path))
            (old/'input/prior-panels.json').write_text('{}')
            frame = pd.DataFrame(dict(x1=[1.]))
            cells = [panel.cell_for(p, frame, t) for p in panel.PANELS for t in panel.THREADS]
            (old/'manifest.json').write_text(json.dumps(dict(cells=cells, datasets=datasets, panels=panel.PANELS,
                                                             policy=dict(repetitions=3), timed_fits=0)))
            new.mkdir()
            panel.reuse(new, old, save_fe=False, panels=['long'])
            manifest = json.loads((new/'manifest.json').read_text())
            self.assertEqual(list(manifest['panels']), ['long'])
            self.assertEqual({c['design'] for c in manifest['cells']}, {'veneto-long'})
            self.assertTrue(all(c['id'].endswith('-fe0') and not c['save_fe'] for c in manifest['cells']))
            self.assertEqual(manifest['timed_fits'], 38*panel.REPETITIONS)
            self.assertFalse(manifest['policy']['save_fe'])
            with self.assertRaises(ValueError):
                panel.reuse(Path(d)/'other', old, panels=['middle'])

    def test_manifest_ladder_and_license(self):
        frame = pd.DataFrame(dict(x1=[1.], y=[2.]))
        cells = [panel.cell_for(p, frame, t) for p in panel.PANELS for t in panel.THREADS]
        self.assertEqual(panel.THREADS, [1, 2, 4, 8, 16])
        self.assertEqual(sum(len(c['arms']) for c in cells)*panel.REPETITIONS, 228)
        self.assertTrue(all(('reghdfe' in c['arms']) == (c['threads'] <= 4) for c in cells))
        self.assertTrue(all({'pyfixest-within', 'julia', 'julia-strict'} <= set(c['arms'])
                            and 'pyfixest' not in c['arms'] for c in cells))
        self.assertEqual(panel.smoke_fits(), 16)
        # Real panels keep their own control counts without a synthetic suffix.
        self.assertEqual(panel.dataset_key(dict(cells[0], rows=771)), 'ols-veneto-long-n771')
        unsaved = panel.cell_for('long', frame, 4, save_fe=False)
        self.assertEqual((unsaved['id'], unsaved['save_fe']), ('veneto-long-t4-fe0', False))
        self.assertEqual(unsaved['arms'], panel.cell_for('long', frame, 4)['arms'])

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
