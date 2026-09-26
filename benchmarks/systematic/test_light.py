import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from common import HERE, arm_order, cell_id, cells, read_json, sha256, write_json
from light import active_arms, choose_fitting, configure_group, groups, historical_costs, pack, skips, smoke_groups
from campaign import build_plan
from light_report import pair_eligible
from report import quantiles


class LightTests(unittest.TestCase):
    def setUp(self):
        self.manifest=read_json(HERE/'manifest-light-v2.json')
        self.history=HERE.parent/'reports/systematic-stopped-2026-09-11/runs.csv'

    def test_frozen_grid_and_orders(self):
        grid=cells(self.manifest);allgroups=groups(self.manifest)
        self.assertEqual(len(grid),140);self.assertEqual(len(allgroups),568)
        self.assertEqual(len({(cell_id(g['cell']),g['repetition']) for g in allgroups}),568)
        self.assertEqual(sum(g['repetition']==0 for g in allgroups),8)
        self.assertEqual(len(smoke_groups(self.manifest)),28)
        for group in allgroups:
            arms=active_arms(group,self.manifest)
            for arm in arms[1:]:
                self.assertEqual(sum(arm_order(arms,r).index(arms[0])<arm_order(arms,r).index(arm) for r in range(1,5)),2)

    def test_history_and_complete_packing(self):
        self.assertEqual(sha256(self.history),self.manifest['historical_cost_sha256'])
        records=historical_costs(self.history)
        prepared=[configure_group(g,self.manifest,records) for g in groups(self.manifest)]
        bundles=pack(prepared,37800,43200)
        self.assertEqual(sum(len(b['groups']) for b in bundles),568)
        self.assertTrue(all({g['cell']['model'] for g in b['groups']}=={'ols','ppml'} for b in bundles))
        largest=max(b['predicted_seconds'] for b in bundles)
        self.assertLess(largest,43200)
        print('PREDECLARED PACKING',len(bundles),sum(g['predicted_seconds'] for g in prepared)/3600,'node hours')

    def test_tail_fill_and_deadline(self):
        large={'limits':{'a':dict(startup=600,command=2400,extraction=600)}}
        small={'limits':{'a':dict(startup=120,command=60,extraction=60)}}
        self.assertEqual(choose_fitting([large,small],1500),1)
        self.assertIsNone(choose_fitting([large,small],1000))

    def test_explicit_exclusions(self):
        excluded=[g for g in groups(self.manifest) if any(v['status']=='budget_exclusion' for v in skips(g['cell'],g['repetition'],self.manifest).values())]
        self.assertEqual(len(excluded),8)
        self.assertTrue(all(self.manifest['arms'][g['cell']['model']][0] in active_arms(g,self.manifest) for g in groups(self.manifest)))

    def test_four_repetitions_and_pair_science(self):
        records=[dict(repetition=r,eligible=True,seconds=r) for r in range(1,5)]
        self.assertEqual(quantiles(records,'seconds',4)[1],2.5)
        self.assertIsNone(quantiles(records[:3],'seconds',4))
        comparison=dict(status='pass',checks=dict(sample_exact=True))
        self.assertTrue(pair_eligible(dict(oracle_checks=None),comparison,True,True))
        for integrity,smoke in [(False,True),(True,False)]:
            self.assertFalse(pair_eligible(dict(oracle_checks=None),comparison,integrity,smoke))
        self.assertFalse(pair_eligible(dict(oracle_checks={'coef':False}),comparison,True,True))
        self.assertFalse(pair_eligible({},dict(status='pass',checks={}),True,True))

    def test_startup_timeout_kills_worker_group(self):
        import os
        import runner
        import subprocess
        import sys
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);data=root/'input.dta';data.write_bytes(b'fixture')
            cell=next(c for c in cells(self.manifest) if c['model']=='ols')
            real_popen=subprocess.Popen
            children=[]
            def fake_popen(*args,**kwargs):
                child=real_popen([sys.executable,'-c','import time;time.sleep(60)'],**kwargs)
                children.append(child);return child
            with patch.object(runner.os,'sched_getaffinity',return_value={0,1,2,3},create=True),patch.object(runner.subprocess,'Popen',side_effect=fake_popen):
                record=runner.measure(root,cell,'fixest','warm',data,data,root/'out',self.manifest,'id',dict(startup=.03,command=1,extraction=1))
            self.assertEqual(record['status'],'startup_timeout')
            self.assertIsNotNone(children[0].poll())
            self.assertFalse((root/'worker-fixest').exists())


if __name__=='__main__':unittest.main()
