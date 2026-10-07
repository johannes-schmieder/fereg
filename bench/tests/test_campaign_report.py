"""Final summaries must not promote partial comparisons or worker drift."""
import copy
import sys
from pathlib import Path
import unittest

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from campaign import POLICY
from campaign_report import summarize, planned_arms
from campaign_collect import accounting_records


def fixture():
    cells = [dict(id=f'cell-{t}', model='ols', design='akm-baseline', rows=1000000,
                  threads=t, vce='iid', save_fe=False, section='baseline' if t == 4 else 'scaling')
             for t in [4, 16]]
    capture = dict(N=1000000, sample_sha256='identical', finite_slope_rank=16, omitted=[],
                   singleton_deletions=0, separation_deletions=0, absorbed_dof=200000,
                   clusters=None, covariance_policy='stock-ols-iid',
                   beta=[1.0]*16, covariance=np.eye(16).tolist())
    records, comparisons = [], []
    for cell in cells:
        for arm in planned_arms(cell):
            for rep in [1, 2, 3]:
                records.append(dict(cell=cell['id'], arm=arm, repetition=rep, status='ok',
                                    metadata=dict(seconds=2 if arm == 'fereg' else 6, rss=1,
                                                  fereg_threads=cell['threads']),
                                    capture=copy.deepcopy(capture), host='same',
                                    estimator_tree_rss_bytes=2**30, memory_sweeps=20))
                if arm != 'fereg':
                    comparisons.append(dict(cell=cell['id'], arm=arm, repetition=rep,
                                            eligible=True, reasons=[]))
    return dict(manifest=dict(cells=cells, policy=POLICY), records=records, comparisons=comparisons)


class CampaignReportTests(unittest.TestCase):
    def test_ratio_requires_all_paired_repetitions(self):
        campaign = fixture()
        result = summarize(campaign)
        self.assertTrue(result.eligible.all())
        self.assertEqual(set(result[result.arm != 'fereg'].speedup), {3.0})
        campaign['comparisons'][0].update(eligible=False, reasons=['covariance'])
        result = summarize(campaign)
        row = result[(result.cell == 'cell-4') & (result.arm == 'fixest')].iloc[0]
        self.assertFalse(row.eligible)
        self.assertTrue(np.isnan(row.speedup))
        self.assertIn('covariance', row.reasons)

    def test_worker_drift_excludes_higher_core_comparison(self):
        campaign = fixture()
        for record in campaign['records']:
            if record['cell'] == 'cell-16' and record['arm'] == 'fereg':
                record['capture']['beta'][0] += .01
        result = summarize(campaign)
        self.assertFalse(result[result.cell == 'cell-16'].eligible.any())
        self.assertTrue(result[result.cell == 'cell-4'].eligible.all())
        self.assertTrue(all('worker beta' in x for x in result[result.cell == 'cell-16'].reasons))

    def test_short_rss_observation_is_unresolved(self):
        campaign = fixture()
        campaign['records'][0]['memory_sweeps'] = 2
        result = summarize(campaign)
        row = result[(result.cell == 'cell-4') & (result.arm == 'fereg')].iloc[0]
        self.assertTrue(row.eligible)
        self.assertFalse(row.memory_resolved)
        self.assertTrue(np.isnan(row.rss_gib))

    def test_native_repeatability_is_a_gate(self):
        campaign = fixture()
        campaign['records'][0]['capture']['beta'][0] += 1e-13
        result = summarize(campaign)
        self.assertFalse(result[result.cell == 'cell-4'].eligible.any())
        self.assertTrue(all('native repeatability' in x for x in result[result.cell == 'cell-4'].reasons))

    def test_incomplete_scheduler_record_is_rejected(self):
        with self.assertRaises(ValueError):
            accounting_records('jobnumber 3\nfailed 0\nexit_status 0\n')


if __name__ == '__main__':
    unittest.main()
