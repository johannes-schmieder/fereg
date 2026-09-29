import copy
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest

import run


class PublicGateTests(unittest.TestCase):
    def cg(self):
        names = ['probe-direct','probe-cmg'] + ['control'+str(i) for i in range(10)]
        tests=[]
        for kind,ts,cs in [('direct',[1,4],[1,4,64,65,129]),('cmg',[1,8],[0,1,2,3])]:
            cases=[dict(kind=kind,threads=t,frame=w,case=c,passed=True)
                   for t in ts for w in range(4) for c in cs]
            tests.append(dict(name='probe-'+kind,cases=cases))
        tests += [dict(name=name,passed=True,returncode=0) for name in names[2:]]
        arm=dict(name='nonretaining-cg',job_id='10',markers=['COMPLETE','PASS'],test_names=names)
        summary=dict(job_id='10',baseline_failure_reproduced=True,candidate_probes_passed=True,
                     controls_passed=True,ready_for_public_validation=True,
                     arms=dict(candidate=dict(tests=tests),baseline=dict(tests=copy.deepcopy(tests))))
        summary['arms']['baseline']['tests'][0]['cases'][0]['passed']=False
        return arm,summary

    def schwarz(self):
        arm=dict(name='schwarz-imbalance',job_id='11',markers=['COMPLETE','PASS'],required_tests=['test0','test1'])
        summary=dict(job_id='11',all_passed=True,distinct_tests=7,
                     tests=[dict(passed=True,process_rc=0,expected=['test'+str(i)]) for i in range(7)])
        return arm,summary

    def test_passed_core_gates(self):
        for arm,summary in [self.cg(),self.schwarz()]:
            self.assertTrue(run.core_passed(arm,summary,dict(status='pass'),'COMPLETE\nPASS\n'))

    def test_failed_pending_accounting_or_bad_markers_never_build(self):
        for arm,summary in [self.cg(),self.schwarz()]:
            for status in ['failed','pending']:
                self.assertFalse(run.core_passed(arm,summary,dict(status=status),'COMPLETE\nPASS\n'))
            for log in ['','COMPLETE\n','COMPLETE\nPASS\nPASS\n']:
                self.assertFalse(run.core_passed(arm,summary,dict(status='pass'),log))

    def test_missing_duplicate_or_failed_cg_cases_refuse(self):
        for mutation in ['missing','duplicate','failed','control','flag','baseline_missing','no_baseline_failure']:
            arm,summary=self.cg()
            test=summary['arms']['candidate']['tests'][0]
            if mutation=='missing': test['cases'].pop()
            if mutation=='duplicate': test['cases'][-1]=test['cases'][0]
            if mutation=='failed': test['cases'][0]['passed']=False
            if mutation=='control': summary['arms']['candidate']['tests'][-1]['returncode']=101
            if mutation=='flag': summary['ready_for_public_validation']=False
            if mutation=='baseline_missing': summary['arms']['baseline']['tests'][0]['cases'].pop()
            if mutation=='no_baseline_failure': summary['arms']['baseline']['tests'][0]['cases'][0]['passed']=True
            self.assertFalse(run.core_passed(arm,summary,dict(status='pass'),'COMPLETE\nPASS\n'),mutation)

    def test_schwarz_missing_tests_and_false_flags_refuse(self):
        for mutation in ['missing','empty','duplicate','failed','flag']:
            arm,summary=self.schwarz()
            if mutation=='missing': summary['tests'].pop()
            if mutation=='empty': summary['tests'][0]['expected']=[]
            if mutation=='duplicate': summary['tests'][0]['expected']=['test1']
            if mutation=='failed': summary['tests'][0]['process_rc']=101
            if mutation=='flag': summary['all_passed']=False
            self.assertFalse(run.core_passed(arm,summary,dict(status='pass'),'COMPLETE\nPASS\n'),mutation)

    def test_fixture_requires_exact_paths_status_and_stata_rc(self):
        good=dict(completed=True,status='success',tests_failed=0,tests_passed=1,
                  results=[dict(fixture='one.do',status='success',process_rc=0,stata_rc=0)])
        self.assertTrue(run.fixture_passed(good,['one.do']))
        for field,value in [('stata_rc',9),('process_rc',1),('fixture','other.do'),('status','failure')]:
            bad=copy.deepcopy(good);bad['results'][0][field]=value
            self.assertFalse(run.fixture_passed(bad,['one.do']))
        self.assertFalse(run.fixture_passed(good,['one.do','two.do']))
        good['completed']=False
        self.assertFalse(run.fixture_passed(good,['one.do']))

    def test_bounded_launcher_preserves_success_failure_timeout_and_deadline(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)
            for script,expected in [('print("ok")',0),('raise SystemExit(3)',3)]:
                result=run.command([sys.executable,'-c',script],root,os.environ,root/'run.log',2,time.monotonic()+3)
                self.assertEqual(result['status'],'finished')
                self.assertEqual(result['returncode'],expected)
            result=run.command([sys.executable,'-c','import time; time.sleep(5)'],root,os.environ,root/'timeout.log',.05,time.monotonic()+3)
            self.assertEqual(result['status'],'timeout')
            self.assertLess(result['returncode'],0)
            result=run.command(['must-not-launch'],root,os.environ,root/'deadline.log',1,time.monotonic()-1)
            self.assertEqual(result['status'],'controller_deadline')


if __name__=='__main__':
    unittest.main()
