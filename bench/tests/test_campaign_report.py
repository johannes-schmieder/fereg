"""Final summaries must not promote partial comparisons or worker drift."""
import copy
import json
import sys
from pathlib import Path
import unittest
import tempfile

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from campaign import POLICY
from campaign_report import (summarize, planned_arms, comparison_cell, completed_seconds, check_host_observations,
                             production_job, stopped_tasks)
from campaign_collect import accounting_records


def fixture():
    cells = [dict(id=f'cell-{t}', model='ols', design='akm-baseline', rows=1000000, repetitions=3,
                  threads=t, vce='iid', save_fe=False, section='baseline' if t == 4 else 'scaling',
                  arms=['fereg', 'fixest', 'julia', *(['reghdfe'] if t <= 4 else [])])
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
                                                  fereg_threads=cell['threads'],rc='0',complete='1',clock_resolution=1e-9),
                                    process_rc=0,
                                    capture=copy.deepcopy(capture), host='same',
                                    estimator_tree_rss_bytes=2**30, memory_sweeps=20, memory_exact=True,
                                    ready_rss_bytes=2**29, peak_rss_bytes=3*2**29))
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
        self.assertEqual(row.timed_repetitions,3)
        self.assertEqual(comparison_cell(result,'cell-4','fixest'),'6.00*')
        self.assertEqual(comparison_cell(result,'cell-4','fixest','speedup'),'NR')

    def test_completed_science_failure_can_show_one_timing(self):
        campaign=fixture()
        runs=[r for r in campaign['records'] if r['cell']=='cell-4' and r['arm']=='reghdfe']
        runs[0].update(status='capture-error',error='saved predictor fails independent model score')
        for run in runs[1:]:run.update(status='skipped-after-failure',metadata={})
        result=summarize(campaign)
        row=result[(result.cell=='cell-4') & (result.arm=='reghdfe')].iloc[0]
        self.assertFalse(row.eligible)
        self.assertEqual(row.timed_repetitions,1)
        self.assertEqual(row.observed_seconds,6)
        self.assertTrue(np.isnan(row.seconds))
        self.assertIn('6.00*',comparison_cell(result,'cell-4','reghdfe'))
        self.assertIn('n=1',comparison_cell(result,'cell-4','reghdfe'))
        self.assertEqual(comparison_cell(result,'cell-4','reghdfe','speedup'),'NR')

    def test_failed_commands_do_not_supply_completed_timings(self):
        record=fixture()['records'][0]
        self.assertEqual(completed_seconds(record),2)
        for update in [dict(status='error'),dict(status='timeout'),dict(status='command-timeout'),
                       dict(process_rc=1),dict(metadata=dict(record['metadata'],rc='1')),
                       dict(metadata=dict(record['metadata'],complete='0')),
                       dict(metadata=dict(record['metadata'],seconds=float('nan'))),
                       dict(metadata=dict(record['metadata'],clock_resolution=.01))]:
            candidate=copy.deepcopy(record);candidate.update(update)
            self.assertIsNone(completed_seconds(candidate))

    def test_worker_drift_excludes_higher_core_comparison(self):
        campaign = fixture()
        for record in campaign['records']:
            if record['cell'] == 'cell-16' and record['arm'] == 'fereg':
                record['capture']['beta'][0] += .01
        result = summarize(campaign)
        self.assertFalse(result[result.cell == 'cell-16'].eligible.any())
        self.assertTrue(result[result.cell == 'cell-4'].eligible.all())
        self.assertTrue(all('worker beta' in x for x in result[result.cell == 'cell-16'].reasons))

    def test_worker_witness_matches_the_control_count(self):
        campaign = fixture()
        other = dict(campaign['manifest']['cells'][0], id='cell-4-k4', regressors=4)
        campaign['manifest']['cells'].append(other)
        for record in [r for r in campaign['records'] if r['cell'] == 'cell-4']:
            campaign['records'].append(dict(copy.deepcopy(record), cell='cell-4-k4'))
        campaign['comparisons'] += [dict(c, cell='cell-4-k4') for c in campaign['comparisons'] if c['cell'] == 'cell-4']
        result = summarize(campaign)
        self.assertTrue(result[result.cell == 'cell-16'].eligible.all())

    def test_short_rss_observation_is_unresolved(self):
        campaign = fixture()
        row = summarize(campaign).query("cell == 'cell-4' and arm == 'fereg'").iloc[0]
        self.assertEqual((row.memory_method, row.rss_gib, row.rss_increment_gib), ('kernel peak', 1.5, 1.0))
        campaign['records'][0]['memory_exact'] = False
        row = summarize(campaign).query("cell == 'cell-4' and arm == 'fereg'").iloc[0]
        self.assertEqual((row.memory_method, row.rss_gib), ('sampled', 1.0))
        self.assertTrue(np.isnan(row.rss_increment_gib))
        campaign['records'][0]['memory_sweeps'] = 2
        result = summarize(campaign)
        row = result[(result.cell == 'cell-4') & (result.arm == 'fereg')].iloc[0]
        self.assertTrue(row.eligible)
        self.assertFalse(row.memory_resolved)
        self.assertTrue(np.isnan(row.rss_gib))

    def test_convention_agreement_supplies_ratios_with_a_mark(self):
        campaign = fixture()
        for comparison in campaign['comparisons']:
            if comparison['arm'] == 'julia':
                comparison.update(eligible=False, reasons=['absorbed_dof', 'covariance'], agreement='convention',
                                  convention_reasons=['absorbed_dof', 'covariance'], covariance_scale=1.001)
        result = summarize(campaign)
        row = result[(result.cell == 'cell-4') & (result.arm == 'julia')].iloc[0]
        self.assertFalse(row.eligible)
        self.assertEqual((row.agreement, row.speedup, row.covariance_scale), ('convention', 3.0, 1.001))
        self.assertFalse(row.timing_discrepancy)
        self.assertEqual(comparison_cell(result, 'cell-4', 'julia'), '6.00†')
        self.assertEqual(comparison_cell(result, 'cell-4', 'julia', 'speedup'), '3.00')
        campaign['comparisons'][-1].update(agreement='differs')
        row = summarize(campaign).query("cell == 'cell-16' and arm == 'julia'").iloc[0]
        self.assertEqual(row.agreement, 'differs')
        self.assertTrue(np.isnan(row.speedup))

    def test_native_repeatability_is_a_gate(self):
        campaign = fixture()
        campaign['records'][0]['capture']['beta'][0] += 1e-13
        result = summarize(campaign)
        self.assertFalse(result[result.cell == 'cell-4'].eligible.any())
        self.assertTrue(all('native repeatability' in x for x in result[result.cell == 'cell-4'].reasons))

    def test_incomplete_scheduler_record_is_rejected(self):
        with self.assertRaises(ValueError):
            accounting_records('jobnumber 3\nfailed 0\nexit_status 0\n')

    def test_observed_campaign_requires_matching_successful_host_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            context=dict(host_observation_required=True,smoke_job='10',production_job='11')
            manifest=dict(policy=dict(bundles=1))
            accounting=dict(jobs=[dict(jobnumber='10',taskid='undefined',hostname='scc-a.example'),
                                  dict(jobnumber='11',taskid='1',hostname='scc-b.example')])
            for label,job,task,host in [('smoke','10','0','scc-a'),('task-1','11','1','scc-b')]:
                path=root/f'host-{label}'
                path.mkdir()
                (path/'host.json').write_text(json.dumps(dict(isolation_ok=True,errors=[],
                    host=dict(job_id=job,task_id=task,hostname=host))))
            check_host_observations(root,context,manifest,accounting)
            path=root/'host-task-1/host.json'
            original=json.loads(path.read_text())
            for update in [dict(isolation_ok=False),dict(errors=['competing compute']),
                           dict(host=dict(original['host'],job_id='12')),
                           dict(host=dict(original['host'],task_id='2')),
                           dict(host=dict(original['host'],hostname='scc-c'))]:
                path.write_text(json.dumps(dict(original,**update)))
                with self.assertRaises(ValueError):
                    check_host_observations(root,context,manifest,accounting)


    def test_a_resubmitted_task_keeps_its_own_job(self):
        context = dict(production_job='11', task_jobs={'1': '12'},
                       stopped_tasks=[dict(job='11', task='1', reason='wrong processor type')])
        self.assertEqual([production_job(context, t) for t in [1, 2]], ['12', '11'])
        self.assertEqual(stopped_tasks(context), {('11', '1')})
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            context.update(host_observation_required=True, smoke_job='10')
            accounting = dict(jobs=[dict(jobnumber='10', taskid='undefined', hostname='scc-a'),
                                    dict(jobnumber='12', taskid='1', hostname='scc-c'),
                                    dict(jobnumber='11', taskid='2', hostname='scc-b')])
            for label, job, task, host in [('smoke', '10', '0', 'scc-a'), ('task-1', '12', '1', 'scc-c'),
                                           ('task-2', '11', '2', 'scc-b')]:
                (root/f'host-{label}').mkdir()
                (root/f'host-{label}/host.json').write_text(json.dumps(dict(
                    isolation_ok=True, errors=[], host=dict(job_id=job, task_id=task, hostname=host))))
            check_host_observations(root, context, dict(policy=dict(bundles=2)), accounting)
            del context['task_jobs']
            with self.assertRaises(ValueError):
                check_host_observations(root, context, dict(policy=dict(bundles=2)), accounting)


if __name__ == '__main__':
    unittest.main()
