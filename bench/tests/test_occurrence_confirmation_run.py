"""S confirmation uses the frozen S arm and rejects a reused discovery host."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import membership_confirmation_run as run
import occurrence_confirmation as m
import test_membership_confirmation_run as old

class OccurrenceExecution(unittest.TestCase):
    def plan(self):
        p=old.plan();p.update(kind=m.KIND,native_affinity_cores=5)
        p['blocks']=m.schedule([x['id'] for x in p['cells']]);return p
    def test_complete_s_blocks_and_rank_bound(self):
        p=self.plan();rows=[]
        for label,cid,arm,phase,block,position in run.measurement_order(p):
            rows.append(dict(label=label,cell=cid,arm=arm,phase=phase,block=block,position=position,
                             seconds=.9 if arm=='cells_s' else 1.,status='passed'))
        got=run.summarize(p,rows)
        self.assertEqual(got['status'],'passed');self.assertEqual(len(got['block_scores']),192)
        self.assertEqual(got['analysis'],m.ANALYSIS)
        self.assertEqual(run.summarize(p,rows[:-1])['status'],'incomplete')
        for row in rows:
            if row['arm']=='cells_s':row['seconds']=1.1
        self.assertEqual(run.summarize(p,rows)['status'],'failed')
    def test_freeze_binds_s_profile_and_driver_source(self):
        with tempfile.TemporaryDirectory() as directory:
            p=self.plan();driver=run.freeze(run.c.routing.identity(p),Path(directory)/'driver.json',
                                            discovery_unopened=True,profile='occurrence')
            run.validate_driver(p,driver)
            self.assertEqual(driver['kind'],m.KIND+'_execution_v1_driver')
            self.assertIn('bench/tests/test_occurrence_confirmation_run.py',driver['sources'])
            with self.assertRaises(ValueError):run.validate_driver(old.plan(),driver)
    def test_same_host_rejected_before_calls(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(run,'validate_admission'), \
             patch.object(run,'validate_science'), patch.object(run,'check_library_limits'), \
             patch.object(run.c.scc.discovery,'host',return_value={'slots':28,'hostname':'scc-a.scc.bu.edu'}), \
             patch.object(run.c,'native',side_effect=AssertionError('no calls')):
            with self.assertRaisesRegex(ValueError,'distinct physical host'):
                run.measure(self.plan(),{}, {'discovery_host':'scc-a'}, {}, Path(directory)/'out')

if __name__=='__main__':unittest.main()
