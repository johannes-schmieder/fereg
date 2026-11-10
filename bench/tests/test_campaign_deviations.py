import sys,unittest,tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import numpy as np
from unittest.mock import patch
import campaign
from campaign_deviations import compare,comparisons,finite
class DeviationsTests(unittest.TestCase):
 def test_symmetric_profiles_and_diagnostic_budget(self):
  with patch.dict(campaign.POLICY,dict(symmetric_strict=True,prediction_diagnostics=True,command_timeout_seconds=3600,command_timeout_by_arm={})):
   for model in ['ols','ppml']:
    c=dict(model=model,threads=4,save_fe=False)
    selected=campaign.arms(c)
    self.assertIn('fereg-strict',selected)
    for arm in selected:
     if not arm.endswith('-strict'):self.assertIn(arm+'-strict',selected)
    self.assertIn('reghdfejl' if model=='ols' else 'glfixedeffects',selected)
    self.assertEqual(campaign.fit_budget(c,'fereg',1),9000)
    self.assertEqual(campaign.fit_budget(c,'fereg',2),4500)
  with patch.dict(campaign.POLICY,dict(command_timeout_seconds=None,command_timeout_by_arm={})):
   with self.assertRaisesRegex(ValueError,'agreed common timeout'):campaign.command_timeout('fereg')
 def test_scaling_and_sample_mismatch(self):
  a=dict(names=['x1','x2'],beta=[2.,-1.],standard_errors=[.5,2.],covariance=[[.25,0.],[0.,4.]],N=3,sample_sha256='a',omitted_or_nonfinite=[],objective_kind='rss',reported_objective=8.)
  b=dict(a,beta=[2.05,-1.2],standard_errors=[1.,4.],covariance=[[1.,0.],[0.,16.]],N=2,sample_sha256='b')
  with tempfile.TemporaryDirectory() as d:
   p,q=Path(d)/'a',Path(d)/'b';p.mkdir();q.mkdir()
   np.savez(p/'numerical-rows.npz',obs_id=[1,2,3],retained=[True,True,True],prediction=[1.,2.,3.])
   np.savez(q/'numerical-rows.npz',obs_id=[1,2,3],retained=[True,False,True],prediction=[2.,0.,5.])
   r=compare(a,b,p,q)
  self.assertAlmostEqual(r['coefficient_max_reference_se'],.1)
  self.assertAlmostEqual(r['covariance_best_scalar'],4.)
  self.assertEqual(r['covariance_residual_after_scalar'],0.)
  self.assertEqual(r['reference_only'],1)
  self.assertEqual(r['sample_intersection'],2)
  self.assertAlmostEqual(r['prediction_rmse'],np.sqrt(2.5))
  self.assertNotIn('reported_objective_difference',r)
 def test_optional_missing_metadata(self):
  for x in [None,".","",float("nan"),float("inf")]:self.assertIsNone(finite(x))
 def test_undefined_zero_se(self):
  a=dict(names=['x'],beta=[0.],standard_errors=[0.],covariance=[[0.]],N=1,sample_sha256='x',omitted_or_nonfinite=[],objective_kind='rss',reported_objective=1.)
  r=compare(a,a)
  self.assertIsNone(r['coefficient_max_reference_se'])
  self.assertIsNone(r['covariance_relative_frobenius'])
  self.assertEqual(r['reported_objective_difference_per_observation'],0.)

class BlockReportingTests(unittest.TestCase):
 def fixture(self):
  records=[];cells=[]
  for rep,(native,other) in enumerate([(1,2),(100,50),(100,200)],1):
   c=dict(id=f'block{rep}',source_cell='original',source_repetitions=3,block_family='fixest',block_repetition=rep,
          repetitions=1,model='ols',design='akm-baseline',rows=100,threads=4,vce='iid',save_fe=False,regressors=1,section='baseline',arms=['fereg','fixest'])
   cells.append(c)
   for arm,seconds in [('fereg',native),('fixest',other)]:
    records.append(dict(cell=c['id'],arm=arm,repetition=rep,status='ok',host=f'host{rep}',job=str(rep),task=1,process_rc=0,
       numerical_capture=dict(sample_sha256='same'),metadata=dict(seconds=seconds,rc='0',complete='1',clock_resolution=1e-9)))
  return dict(manifest=dict(cells=cells),records=records)
 def test_ratios_use_matched_native_not_pooled_medians(self):
  from campaign_blocks_report import paired_summary
  r=paired_summary(self.fixture()).iloc[0]
  self.assertEqual(r.paired_ratio,2)
  self.assertEqual(r.seconds/r.paired_native_seconds,.5)
 def test_ratio_rejects_cross_allocation_anchor(self):
  from campaign_blocks_report import paired_summary
  c=self.fixture();c['records'][0]['host']='wrong'
  with self.assertRaisesRegex(ValueError,'allocations'):paired_summary(c)
 def test_censored_fit_does_not_become_completed_time(self):
  from campaign_blocks_report import paired_summary
  c=self.fixture();c['records'][-1]['status']='command-timeout'
  r=paired_summary(c).iloc[0]
  self.assertEqual(r.timeouts,1);self.assertEqual(r.timed_repetitions,2);self.assertEqual(r.planned_repetitions,3)

class ExportQualificationTests(unittest.TestCase):
 def test_native_limit_is_profile_specific(self):
  from campaign_export import validate_native_results
  native=[dict(cell='c',arm='fereg',status='ok'),dict(cell='c',arm='fereg-strict',status='command-timeout')]
  self.assertEqual(validate_native_results(native,{'c':[],'c::fereg-strict':['native application unavailable']},{}),{('c','fereg-strict')})
  for changes in [dict(status='capture-error'),dict(status='timeout')]:
   bad=[dict(native[0],**changes),native[1]]
   if changes['status']=='capture-error':
    with self.assertRaises(ValueError):validate_native_results(bad,{}, {})
  with self.assertRaises(ValueError):validate_native_results(native,{'c':['native repeatability']},{})
  with self.assertRaises(ValueError):validate_native_results(native,{'c::fereg-strict':['native cross-block beta']},{})
 def test_veneto_ols_capture_uses_rss(self):
  from campaign_export import capture_hashes,canonical
  record=dict(cell='veneto',arm='fereg',repetition=1,capture={'N':1},metadata={'rss':'2'})
  self.assertEqual(capture_hashes([record],[dict(id='veneto',model='ols')])['veneto/fereg-r1'],canonical(dict(capture={'N':1},objective=2.)))
 def test_strict_stata_timer_resolution_is_qualified(self):
  from campaign_report import completed_seconds
  for arm in ['fereg-strict','areg-strict','reghdfejl','reghdfejl-strict']:
   r=dict(arm=arm,status='capture-error',process_rc=0,metadata=dict(rc='0',complete='1',seconds='1',clock_resolution='.001'))
   self.assertIsNone(completed_seconds(r))
   r['metadata']['clock_resolution']='1e-9'
   self.assertEqual(completed_seconds(r),1.)

class NativeHostRepeatabilityTests(unittest.TestCase):
 def test_every_host_has_its_own_exact_reference(self):
  from campaign_report import check_native_repetitions
  cells=[dict(id='c'+str(i),source_cell='s',model='ols',threads=4) for i in range(3)]
  records=[dict(cell=c['id'],arm='fereg',repetition=i+1,status='ok',host='a' if i==0 else 'b',
               capture=dict(N=1,beta=[1.+i*1e-12],covariance=[[1.]]),metadata=dict(rss=1.,fereg_threads=4)) for i,c in enumerate(cells)]
  policy={k+'_'+suffix:1e-7 for k in ['coefficient','vce','objective'] for suffix in ['rtol','atol']}
  failures=check_native_repetitions(dict(manifest=dict(cells=cells,source_cells=[{}],policy=policy),records=records))
  self.assertEqual(failures['c1'],[])
  self.assertEqual(failures['c2'],['native same-host repeatability'])

if __name__=='__main__':unittest.main()
