"""Campaign guards test science and resource contracts rather than estimator code."""
import sys
from pathlib import Path
import tempfile
import unittest
import shutil
import subprocess
import numpy as np
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import campaign
from campaign import configurations, arms, make_dataset, compare_records, prepare, absorbed_rank, frozen_dataset, dataset_key, POLICY
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
        self.assertEqual({c['regressors'] for c in cells},{4,16,32})
        self.assertTrue(all(c['regressors']==16 or c['section']=='controls' for c in cells))
        for c in cells:
            if c['threads']>4:self.assertFalse({'reghdfe','ppmlhdfe','areg'} & set(arms(c)))
            self.assertEqual(c['repetitions'],3 if c['rows']>1000000 else 5)
        self.assertEqual({c['threads'] for c in cells},{1,2,4,8,16})
        self.assertGreater(sum(c['threads']==4 for c in cells),sum(c['threads']!=4 for c in cells))
        for model,design in POLICY['size_series']:
            series={c['rows'] for c in cells if c['model']==model and c['design']==design and c['threads']==4
                    and not c['save_fe'] and c['regressors']==16 and c['vce']==('iid' if model=='ols' else 'robust')}
            self.assertEqual(series,set(POLICY['sizes']))

    def test_package_and_areg_arms(self):
        for cell in configurations()+configurations(True):
            self.assertIn('pyfixest-within',arms(cell))
            self.assertNotIn('pyfixest',arms(cell))
            self.assertEqual({'julia','julia-strict'}<=set(arms(cell)),cell['model']=='ols')
            self.assertFalse({'julia','julia-strict'} & set(arms(cell)) and cell['model']=='ppml')
            eligible=cell['model']=='ols' and cell['threads']<=4 and not cell['save_fe']
            self.assertEqual('areg' in arms(cell),eligible)
        self.assertEqual(sum('areg' in arms(c) for c in configurations()),21)
        metadata=dict(df_a='239',arm='areg')
        self.assertEqual(absorbed_rank(metadata,dict(model='ols'),None),240.)

    def test_control_counts_join_the_main_grid(self):
        controls=[c for c in configurations() if c['section']=='controls']
        self.assertEqual(sorted((c['model'],c['regressors']) for c in controls),
                         [('ols',4),('ols',32),('ppml',4),('ppml',32)])
        self.assertTrue(all(c['id'].endswith(f"-k{c['regressors']}") and dataset_key(c).endswith(f"-k{c['regressors']}")
                            and c['arms'][0]=='fereg' for c in controls))
        self.assertIn('areg',[c for c in controls if c['model']=='ols'][0]['arms'])

    def test_new_designs_and_strict_pyfixest(self):
        cells=configurations()
        designs={(c['model'],c['design']) for c in cells if c['section']=='design'}
        self.assertIn(('ols','akm-clustered'),designs)
        self.assertIn(('ppml','akm-clustered'),designs)
        self.assertIn(('ppml','gravity'),designs)
        self.assertNotIn(('ols','gravity'),designs)
        gravity=next(c for c in cells if c['design']=='gravity')
        self.assertEqual(gravity['effects'],3)
        for cell in cells+configurations(True):
            # Strict PyFixest accompanies strict fixest in every cell.
            self.assertEqual('pyfixest-within-strict' in arms(cell),'fixest-strict' in arms(cell))
        self.assertIn('pyfixest-within-strict',gravity['arms'])

    def test_new_topologies(self):
        from campaign import clustered_topology,gravity_topology,gravity_outcome
        import pandas as pd
        groups=clustered_topology(20000,np.random.default_rng(3))
        self.assertEqual(len(groups['g1']),20000)
        self.assertEqual(int(groups['g1'].max()),4000)
        sizes=np.bincount(groups['g2'])
        self.assertGreater(sizes.max(),10*np.median(sizes[sizes>0]))
        # The campaign's one-million-row draw once left a single worker-year
        # in a firm, which every command but areg deleted.
        from campaign import POLICY
        groups=clustered_topology(1000000,np.random.Generator(np.random.PCG64(POLICY['seed'])))
        self.assertFalse((np.bincount(groups['g2'])==1).any() or (np.bincount(groups['g1'])==1).any())
        gravity=gravity_topology(1200)
        self.assertEqual({len(v) for v in gravity.values()},{1200})
        self.assertTrue(np.all(np.diff(gravity['g3'])>=0))
        frame=pd.DataFrame(gravity)
        rng=np.random.default_rng(5)
        for j in range(1,5):frame[f'x{j}']=rng.normal(size=len(frame))
        record=gravity_outcome(frame,dict(regressors=4))
        flagged=frame.x4==1
        self.assertTrue((frame.y[flagged]==0).all() and set(frame.x4.unique())<={0.,1.})
        self.assertEqual(record['covariate_separated_rows'],int(flagged.sum()))
        self.assertGreater(record['zero_outcomes'],.3*len(frame))

    def test_gravity_separation_reaches_capture_and_oracle(self):
        import csv
        import pandas as pd
        from campaign import capture
        from campaign_oracle import separation
        cell=dict(next(c for c in configurations(True) if c['design']=='gravity'),save_fe=False)
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            frame,_=make_dataset(cell,root/'data'/(dataset_key(cell)+'.dta'))
            selected,omitted=separation(frame,cell)
            # feppml and ppmlhdfe retained these 1,133 rows in the SCC smoke.
            self.assertEqual((int(selected.sum()),omitted),(1133,['x16']))
            expected=oracle(frame,cell)
            self.assertLess(expected['residual_score'],1e-9)
            self.assertEqual(expected['names'],[f'x{j}' for j in range(1,16)])
            def write(name,coefficients,rows,omissions):
                directory=root/'output'/cell['id']/f'{name}-r1';directory.mkdir(parents=True)
                with (directory/'coefficients.csv').open('w',newline='') as f:
                    w=csv.writer(f);w.writerow(['name','value']);w.writerows(coefficients.items())
                with (directory/'covariance.csv').open('w',newline='') as f:
                    w=csv.writer(f);w.writerow(['row','column','value'])
                    w.writerows([a,b,float(a==b)] for a in coefficients for b in coefficients)
                pd.DataFrame(dict(obs_id=frame.obs_id,retained=rows.astype(int))).to_csv(directory/'rows.csv',index=False)
                metadata=dict(finite_slope_rank=str(16-omissions),slope_omissions=str(omissions),df_a='371',N=str(int(rows.sum())),
                              covariance_policy='stock-ppml-robust',converged='1',ll='-1')
                record=dict(status='ok',directory=str(directory),metadata=metadata,cell=cell['id'],arm=name)
                record['capture']=capture(directory,cell,frame,metadata)
                fitted=np.full(len(frame),np.nan)
                fitted[rows]=expected['fitted'].mean()
                fitted[selected]=expected['fitted']
                np.savez_compressed(directory/'fitted.npz',fitted=fitted)
                return record
            beta=dict(zip(expected['names'],expected['beta']))
            stata=write('stata',{**beta,'o.x16':0.},selected,1)
            fixest=write('fixest',beta,selected,1)
            self.assertEqual(stata['capture']['omitted'],['o.x16'])
            self.assertEqual(compare_records(stata,fixest,cell)['agreement'],'exact')
            # A package that keeps the covariate-separated rows estimates x16.
            kept=selected|(frame.x16==1).to_numpy()
            diverging=write('diverging',{**beta,'x16':-16.},kept,0)
            result=compare_records(stata,diverging,cell)
            self.assertEqual(result['agreement'],'differs')
            self.assertTrue({'N','sample_sha256','omitted','finite_slope_rank','separation_deletions'}<=set(result['reasons']))
            self.assertTrue(result['common_coefficients'])
            # The smoke gate accepts the separated fit and rejects the one that
            # keeps separated rows, and its record serializes.
            from campaign_oracle import validate_smoke
            rows=validate_smoke(dict(cells=[cell],records=[stata,diverging]))
            import json
            json.dumps(rows)
            self.assertEqual([(r['arm'],r['pass']) for r in rows],[('stata',False),('diverging',False)])
            self.assertTrue(all(rows[0]['checks'][k] for k in ['coefficients','fitted','sample']))
            self.assertFalse(rows[1]['checks']['fitted'])
            # Inputs without a cluster variable, such as the Veneto panels, need none.
            capture(Path(stata['directory']),cell,frame.drop(columns=['cluster']),stata['metadata'])
            # Designs without separation keep the strict sample and rank checks.
            with self.assertRaisesRegex(ValueError,'omitted/missing finite coefficient'):
                capture(Path(stata['directory']),dict(cell,design='three-sparse'),frame,stata['metadata'])

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
            first,second=saved_effects(fit,data,2,np.ones(4,dtype=bool))
            np.testing.assert_array_equal(first,[.5,-1.,-1.,2.])
            np.testing.assert_array_equal(second,[0.,.25,0.,.25])
        class Missing:
            def fixef(self,atol,btol):
                return {'C(g1)':{'1':.5,'2':-1.},'C(g2)':{'20':.25}}
        everything=np.ones(4,dtype=bool)
        with self.assertRaises(ValueError):saved_effects(Missing(),data,2,everything)
        # A level that only separated rows carry has no estimated effect.
        first,second=saved_effects(Missing(),data,2,np.array([True,True,True,False]))
        np.testing.assert_array_equal(first,[.5,-1.,-1.,np.nan])
        np.testing.assert_array_equal(second,[0.,.25,0.,np.nan])
        class OneLevel:
            def fixef(self,atol,btol):
                return {'C(g1)':{'1':.5,'2':-1.,'3':2.}}
        _,second=saved_effects(OneLevel(),data.assign(g2=10.),2,everything)
        np.testing.assert_array_equal(second,[0.,0.,0.,0.])

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

    def test_each_design_shares_one_node_and_each_node_starts_with_the_reference(self):
        import json
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);prepare(root)
            plan=json.loads((root/'manifest.json').read_text())
            self.assertEqual(len(plan['bundles']),POLICY['bundles'])
            for task,bundle in enumerate(plan['bundles'],1):
                self.assertEqual(bundle[0]['section'],'reference')
                self.assertEqual(bundle[0]['id'],f'ols-akm-baseline-n1000000-t4-iid-fe0-ref{task}')
                self.assertEqual((bundle[0]['arms'],bundle[0]['repetitions']),(['fereg'],3))
                self.assertEqual(sum(c['section']=='reference' for c in bundle),1)
                for key in {campaign.series_key(c) for c in bundle[1:]}:
                    sizes=[c['rows'] for c in bundle[1:] if campaign.series_key(c)==key]
                    self.assertEqual(sizes,sorted(sizes))
            for cell in configurations():
                owners={i for i,b in enumerate(plan['bundles']) for c in b if c['section']!='reference'
                        and c['model']==cell['model'] and c['design']==cell['design']}
                self.assertEqual(len(owners),1,cell['id'])
            self.assertEqual(plan['timed_fits'],sum(len(c['arms'])*c['repetitions'] for c in plan['cells']))
            self.assertEqual(len(plan['cells']),len(configurations())+POLICY['bundles'])

    def test_a_partial_run_selects_designs_and_keeps_the_reference(self):
        import json
        from unittest import mock
        with tempfile.TemporaryDirectory() as d, mock.patch.dict(POLICY,sections=['design'],designs=['akm-clustered'],bundles=1):
            root=Path(d);prepare(root)
            plan=json.loads((root/'manifest.json').read_text())
            self.assertEqual([c['id'] for c in plan['cells']],
                             ['ols-akm-baseline-n1000000-t4-iid-fe0-ref1','ols-akm-clustered-n1000000-t4-iid-fe0',
                              'ppml-akm-clustered-n1000000-t4-robust-fe0'])
            self.assertEqual(plan['policy']['designs'],['akm-clustered'])

    def test_a_timeout_ends_its_size_series_on_the_node(self):
        import json
        from unittest import mock
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            cells=[dict(c,rows=rows,id=c['id'].replace('n1200',f'n{rows}'),repetitions=2,
                        arms=['fereg','fixest','reghdfe'])
                   for c in configurations(True)[:1] for rows in [1200,2400]]
            (root/'manifest.json').write_text(json.dumps(dict(bundles=[cells])))
            def fake_fit(taskroot,cell,arm,repetition,frame,cpus):
                status='command-timeout' if arm=='reghdfe' else 'ok'
                return dict(cell=cell['id'],arm=arm,repetition=repetition,status=status,metadata=dict(seconds='1'))
            with mock.patch.object(campaign,'verify_runtime'),mock.patch.object(campaign,'physical_cpus',return_value=list(range(28))),\
                 mock.patch.object(campaign,'fit',side_effect=fake_fit),\
                 mock.patch.object(campaign,'compare_records',return_value=dict(eligible=True,reasons=[],agreement='exact')):
                campaign.run_bundle(root,1)
            records=json.loads((root/'task-1/results.json').read_text())['records']
            status={(r['cell'],r['arm'],r['repetition']):r['status'] for r in records}
            small,large=cells[0]['id'],cells[1]['id']
            self.assertEqual([status[(small,'reghdfe',r)] for r in [1,2]],['command-timeout','skipped-after-failure'])
            self.assertEqual([status[(large,'reghdfe',r)] for r in [1,2]],['skipped-after-timeout']*2)
            self.assertEqual([status[(large,'fixest',r)] for r in [1,2]],['ok','ok'])
            self.assertTrue(all(r['timed_out_rows']==1200 for r in records if r['status']=='skipped-after-timeout'))

    def test_kernel_peak_brackets_the_timed_command(self):
        from unittest import mock
        script=('import pathlib,time,sys\n'
                'd=pathlib.Path(sys.argv[1])\n'
                '(d/"READY").write_text("r")\n'
                'while not (d/"GO").exists():time.sleep(.001)\n'
                '(d/"DONE").write_text("d")\n'
                'while not (d/"ACK").exists():time.sleep(.001)\n')
        with tempfile.TemporaryDirectory() as d:
            directory=Path(d)
            memory={'VmRSS':100,'VmHWM':300}
            with mock.patch.object(campaign,'placement_metadata',return_value=dict(mask='0',memory_nodes=[0])),\
                 mock.patch.object(campaign,'bound_command',side_effect=lambda command,placement:command),\
                 mock.patch.object(campaign,'reset_peak'),\
                 mock.patch.object(campaign,'process_memory',side_effect=lambda pid,field:memory[field]):
                result=campaign.execute([sys.executable,'-c',script,str(directory)],directory,[0],60)
            self.assertEqual(result['status'],'ok')
            self.assertTrue(result['memory_exact'])
            self.assertEqual((result['ready_rss_bytes'],result['peak_rss_bytes']),(100,300))
            directory=Path(d)/'failed';directory.mkdir()
            with mock.patch.object(campaign,'placement_metadata',return_value=dict(mask='0',memory_nodes=[0])),\
                 mock.patch.object(campaign,'bound_command',side_effect=lambda command,placement:command),\
                 mock.patch.object(campaign,'reset_peak',side_effect=PermissionError('denied')),\
                 mock.patch.object(campaign,'process_memory',side_effect=lambda pid,field:memory[field]):
                result=campaign.execute([sys.executable,'-c',script,str(directory)],directory,[0],60)
            self.assertEqual(result['status'],'ok')
            self.assertFalse(result['memory_exact'])

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

    def test_counting_conventions_are_separated_from_numerical_differences(self):
        cell=configurations()[0]
        capture=dict(beta=[1.]*16,covariance=(np.eye(16)+.1).tolist(),N=1000,sample_sha256='a',omitted=[],
                     finite_slope_rank=16,singleton_deletions=0,separation_deletions=0,clusters=None,
                     covariance_policy='stock-ols-iid',absorbed_dof=200,residual_dof=784,correction_dof=784)
        a=dict(status='ok',capture=capture,metadata={'rss':'1'})
        self.assertEqual(compare_records(a,a,cell)['agreement'],'exact')
        # One more absorbed level scales the covariance by 784/783.
        levels=dict(capture,absorbed_dof=201,residual_dof=783,correction_dof=783,
                    covariance=((np.eye(16)+.1)*784/783).tolist())
        result=compare_records(a,dict(a,capture=levels),cell)
        self.assertFalse(result['eligible'])
        self.assertEqual(result['agreement'],'convention')
        self.assertAlmostEqual(result['covariance_scale'],784/783)
        self.assertEqual(result['convention_reasons'],['absorbed_dof','correction_dof','covariance','residual_dof'])
        for changed in [dict(beta=[1.+1e-5]*16),dict(covariance=((np.eye(16)+.1)*1.02).tolist()),
                        dict(covariance=(np.eye(16)+.1+np.diag([1e-4]*16)).tolist())]:
            result=compare_records(a,dict(a,capture=dict(levels,**changed)),cell)
            self.assertEqual(result['agreement'],'differs',changed)
        self.assertEqual(compare_records(a,dict(a,status='timeout'),cell)['agreement'],'unavailable')

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
