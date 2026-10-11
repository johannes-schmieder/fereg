"""Campaign guards test science and resource contracts rather than estimator code."""
import sys
from pathlib import Path
import tempfile
import unittest
import shutil
import subprocess
import numpy as np
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from campaign import configurations, arms, make_dataset, compare_records, prepare, absorbed_rank
from campaign_oracle import oracle


class CampaignTests(unittest.TestCase):
    def test_fixest_saved_fe_large_numeric_ids(self):
        executable=shutil.which('Rscript')
        if not executable:self.skipTest('Rscript unavailable; SCC setup runs this with pinned fixest')
        available=subprocess.run([executable,'-e','quit(status=if(requireNamespace("fixest",quietly=TRUE)) 0 else 2)'],capture_output=True,timeout=30)
        if available.returncode==2:self.skipTest('fixest unavailable; SCC setup runs this with pinned fixest')
        self.assertEqual(available.returncode,0,available.stderr.decode())
        root=Path(__file__).resolve().parents[1]
        result=subprocess.run([executable,str(root/'tests/saved_fe_fixest.R'),str(root)],capture_output=True,text=True,timeout=60)
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)
        self.assertIn('SAVED_FE_FIXEST_TEST_PASS',result.stdout)

    def test_license_and_matrix(self):
        cells=configurations()
        self.assertEqual(len(cells),len({c['id'] for c in cells}))
        self.assertTrue(all(c['regressors']==16 for c in cells))
        for c in cells:
            if c['threads']>4:self.assertFalse({'reghdfe','ppmlhdfe'} & set(arms(c)))
        self.assertEqual({c['threads'] for c in cells},{1,4,8,16})
        self.assertGreater(sum(c['threads']==4 for c in cells),sum(c['threads']!=4 for c in cells))

    def test_strict_ols_profiles_are_frozen_separately(self):
        from campaign_report import planned_arms
        for cell in configurations()+configurations(True):
            self.assertEqual(arms(cell),planned_arms(cell))
            self.assertEqual(len(arms(cell)),len(set(arms(cell))))
            if cell['model']=='ols':
                self.assertIn('fixest',arms(cell))
                self.assertIn('fixest-strict',arms(cell))
        historical=dict(configurations()[0])
        del historical['arms']
        self.assertNotIn('fixest-strict',planned_arms(historical))

    def test_full_baseline_stays_in_one_bundle(self):
        import json
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);prepare(root)
            plan=json.loads((root/'manifest.json').read_text())
            for model in ['ols','ppml']:
                owners={i for i,b in enumerate(plan['bundles']) for c in b
                        if c['model']==model and c['design']=='akm-baseline' and c['rows']==1000000}
                self.assertEqual(len(owners),1)

    def test_scientific_failures_are_unranked(self):
        cell=configurations()[0]
        capture=dict(beta=[1]*16,covariance=np.eye(16).tolist(),N=100,sample_sha256='a',omitted=[],
                     finite_slope_rank=16,singleton_deletions=0,separation_deletions=0,clusters=None,covariance_policy='stock-ols-iid',
                     residual_dof=64,correction_dof=64)
        capture['absorbed_dof']=20
        a=dict(status='ok',capture=capture,metadata={'rss':'1'})
        self.assertTrue(compare_records(a,a,cell)['eligible'])
        for key,value in [('sample_sha256','b'),('N',101),('omitted',['x1']),('beta',[2]*16),('absorbed_dof',21)]:
            b=dict(a,capture=dict(capture,**{key:value}))
            self.assertFalse(compare_records(a,b,cell)['eligible'])
        self.assertFalse(compare_records(a,dict(a,status='timeout'),cell)['eligible'])
        self.assertFalse(compare_records(a,dict(a,capture={k:v for k,v in capture.items() if k!='finite_slope_rank'}),cell)['eligible'])

    def test_independent_oracle_and_lossless_inputs(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            for cell in configurations(True)[:5]:
                frame,meta=make_dataset(cell,root/(cell['id']+'.dta'))
                self.assertTrue(np.array_equal(frame.cluster,frame.g1))
                expected=oracle(frame,cell)
                self.assertLess(expected['residual_score'],1e-9)
                self.assertTrue(np.isfinite(expected['covariance']).all())
                self.assertGreater(meta['topology']['first_pair_rank'],0)

    def test_ppml_count_conventions_match_independent_fe_rank(self):
        with tempfile.TemporaryDirectory() as d:
            cell=next(c for c in configurations(True) if c['model']=='ppml')
            frame,_=make_dataset(cell,Path(d)/'input.dta')
            expected=oracle(frame,cell)['rank']
            self.assertEqual(expected,251)
            # Published counts observed in the SCC smoke, with identical data.
            for raw,native,vce,policy in [(250,'1','robust','stock-ppml-robust'),
                                         (10,'1','cluster','stock-ppml-cluster'),
                                         (251,'0','robust','stock-ppml-robust'),
                                         (11,'0','cluster','stock-ppml-cluster'),
                                         (251,'0','cluster','ppml-cluster-K.adj=FALSE')]:
                meta=dict(df_a=raw,native_arm=native,covariance_policy=policy)
                self.assertEqual(absorbed_rank(meta,dict(cell,vce=vce),frame),expected)


if __name__=='__main__':unittest.main()
