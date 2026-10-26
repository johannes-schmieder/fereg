"""Campaign guards test science and resource contracts rather than estimator code."""
import sys
from pathlib import Path
import tempfile
import unittest
import shutil
import subprocess
import numpy as np
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from campaign import configurations, arms, make_dataset, compare_records, prepare, absorbed_rank, frozen_dataset
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

    def test_pyfixest_and_areg_arms(self):
        for cell in configurations()+configurations(True):
            self.assertTrue({'pyfixest','pyfixest-within'}<=set(arms(cell)))
            one_fe=cell['model']=='ols' and cell['effects']==1 and cell['threads']<=4 and not cell['save_fe']
            self.assertEqual('areg' in arms(cell),one_fe)
        self.assertEqual(sum('areg' in arms(c) for c in configurations()),3)
        metadata=dict(df_a='239',arm='areg')
        self.assertEqual(absorbed_rank(metadata,dict(model='ols'),None),240.)

    def test_pyfixest_saved_effects_fill_the_omitted_reference(self):
        import pandas as pd
        from campaign_pyfixest import saved_effects
        data=pd.DataFrame(dict(g1=[1.,2.,2.,3.],g2=[10.,20.,10.,20.]))
        class Release:
            def fixef(self,atol,btol):
                return {'C(g1)':{'1':.5,'2':-1.,'3':2.},'C(g2)':{'20':.25}}
        class Main:
            def fixef(self,atol,btol):
                return pd.DataFrame(dict(variable=['g1']*3+['g2']*2,level=['1','2','3','10','20'],
                                         coefficient=[.5,-1.,2.,0.,.25]))
        for fit in [Release(),Main()]:
            first,second=saved_effects(fit,data,2)
            np.testing.assert_array_equal(first,[.5,-1.,-1.,2.])
            np.testing.assert_array_equal(second,[0.,.25,0.,.25])
        class Missing:
            def fixef(self,atol,btol):
                return {'C(g1)':{'1':.5,'2':-1.},'C(g2)':{'20':.25}}
        with self.assertRaises(ValueError):saved_effects(Missing(),data,2)

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
        strict=dict(a,capture=dict(capture,covariance_policy='ols-iid-K.exact=TRUE'))
        self.assertTrue(compare_records(a,strict,cell)['eligible'])
        strict['capture']['correction_dof']=63
        self.assertIn('correction_dof',compare_records(a,strict,cell)['reasons'])
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

    def test_frozen_inputs_preserve_bytes_and_reject_changed_evidence(self):
        import copy
        import json
        from campaign import dataset_key, sha256
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            source=root/'previous/task-1/data'
            source.mkdir(parents=True)
            destination=root/'current'
            destination.mkdir()
            cell=configurations(True)[0]
            name=dataset_key(cell)+'.dta'
            frame,metadata=make_dataset(cell,source/name)
            inventory=dict(remote_root=str(root/'previous'),datasets={dataset_key(cell):metadata})
            copied,actual=frozen_dataset(cell,destination/name,inventory)
            self.assertEqual(actual,metadata)
            self.assertTrue(np.array_equal(frame.to_numpy(),copied.to_numpy()))
            for suffix in ['.dta','.json','.components.npz']:
                self.assertEqual(sha256((source/name).with_suffix(suffix)),
                                 sha256((destination/name).with_suffix(suffix)))
            changed=copy.deepcopy(inventory)
            changed['datasets'][dataset_key(cell)]['rows']+=1
            with self.assertRaisesRegex(RuntimeError,'identity mismatch'):
                frozen_dataset(cell,destination/name,changed)
            changed=copy.deepcopy(metadata)
            changed['column_sha256']['y']='0'*64
            (source/name).with_suffix('.json').write_text(json.dumps(changed))
            inventory['datasets'][dataset_key(cell)]=changed
            with self.assertRaisesRegex(RuntimeError,'decoded frozen input mismatch'):
                frozen_dataset(cell,destination/name,inventory)

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
