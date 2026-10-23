"""Zero-native witnesses for the exact diagnosed 30+50 continuation."""
import copy
from pathlib import Path
from unittest.mock import patch
import unittest
import routing

import ownership_diagnostic_eligibility as e
import ownership_qualification_diagnostics as d
import ownership_diagnostic_transport as t
import qualify as q
import test_ownership_diagnostic_transport as first
import test_ownership_qualification_diagnostics as original


class Continuation(unittest.TestCase):
    setUp=original.Runner.setUp
    execute=original.Runner.execute
    prefix=first.Continuation.prefix

    def failed_thirty(self):
        source=self.f.source
        with source.open('rb') as stream:
            q.case_header(stream);header=source.read_bytes()[:stream.tell()]
        words=self.f.data.view('uint64')
        source.write_bytes(header+b''.join(b'0\n'+b''.join(f'{int(v):016x}\n'.encode() for v in column) for column in words))
        self.prefix()
        actual=d.mechanism_evidence
        def legacy(family, workers, calls, request):
            if family=='ownership-inactive-zero' and workers==4:raise ValueError(e.ERROR)
            return actual(family,workers,calls,request)
        with patch.object(d,'mechanism_evidence',side_effect=legacy):
            with self.assertRaisesRegex(ValueError,e.ERROR):
                d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        old=q.read(self.output/'results.json')
        self.assertEqual((len(old['calls']),len(old['cells']),len(self.f.calls)),(30,5,30))
        self.old_output=self.output
        self.output=self.output.with_name('continued')
        self.plan['eligibility_amendment']=dict(charged_seconds=6)
        self.validation=patch.object(e,'validate',return_value=(dict(previous_results=self.old_output/'results.json'),old,{}))
        self.validation.start();self.addCleanup(self.validation.stop)
        return old

    def test_revalidate_all_thirty_then_only_fifty_new_and_complete_proof(self):
        old=self.failed_thirty();old_hash=q.digest(self.old_output/'results.json')
        before=self.f.oracle.call_count
        result=d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),80)
        self.assertEqual(result['calls'][:30],old['calls'])
        self.assertEqual((result['native_calls_reused'],result['native_calls_executed_here'],result['charged_prior_seconds']),(30,50,8))
        self.assertEqual(result['elapsed_s'],8+result['continuation_elapsed_s'])
        self.assertEqual(self.f.oracle.call_count-before,69)  # original inline +18 banks +50 new
        self.assertEqual(sum('reference' in c for c in result['calls']),48)
        self.assertEqual(result['cells']['ownership-inactive-zero-t4']['baseline_construction_required'],False)
        proof=d.prove(self.plan_path,self.output/'results.json',self.output/'proof.json')
        self.assertEqual((proof['status'],proof['native_calls_executed_here']),('passed',50))
        self.assertEqual(q.digest(self.old_output/'results.json'),old_hash)
        changed=copy.deepcopy(result);changed['calls'][20]['seconds']+=.001
        q.write(self.output/'results.json',changed)
        with self.assertRaisesRegex(ValueError,'imported 30-call cohort'):
            d.prove(self.plan_path,self.output/'results.json',self.output/'bad-proof.json')
        self.assertEqual(len(self.f.calls),80)

    def test_prefix_log_drift_stops_before_first_new_native(self):
        old=self.failed_thirty();log=self.old_output/old['calls'][29]['log']
        log.write_text(log.read_text()+'changed\n')
        with self.assertRaisesRegex(ValueError,'raw diagnostic evidence'):
            d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),30)
        self.assertEqual(q.read(self.output/'results.json')['status'],'failed')
        self.assertEqual(q.read(self.old_output/'results.json')['status'],'failed')

    def test_full_column_reconstruction_failure_stops_before_native(self):
        self.failed_thirty()
        with patch.object(e,'reconstruct',side_effect=ValueError('full-column witness')):
            with self.assertRaisesRegex(ValueError,'full-column witness'):
                d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),30)

    def test_wrong_host_cannot_run_fifty(self):
        old=self.failed_thirty();old['host']['host']['hostname']='other'
        with self.assertRaisesRegex(ValueError,'original physical host'):
            d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=original.Monitor)
        self.assertEqual(len(self.f.calls),30)


class Bindings(unittest.TestCase):
    setUp=Continuation.setUp
    execute=Continuation.execute
    prefix=Continuation.prefix
    failed_thirty=Continuation.failed_thirty
    # Only the fixture setup is reused; avoid repeating inherited runner tests.
    def bound(self):
        old=self.failed_thirty();self.validation.stop()
        old['continuation_elapsed_s']=5.1;old['elapsed_s']=7.1
        qualification=self.f.root/'prior-qualification.json';q.write(qualification,self.manifest)
        prior={**self.plan,'schema':1,'kind':d.KIND,'native_calls':80,'timeout_s':120,'retries':0,
               'observer':'desktop','qualification':str(qualification),
               'qualification_sha256':q.digest(qualification),'qualification_identity':routing.identity(self.manifest),
               'execution_sources':dict(e.PREVIOUS_SOURCES)}
        prior.pop('eligibility_amendment')
        prior['transport_repair']['files']={}
        previous=self.f.root/'prior-plan.json';q.write(previous,prior)
        old['plan_sha256']=q.digest(previous);old['qualification_sha256']=prior['qualification_sha256']
        old['transport_repair']=copy.deepcopy(prior['transport_repair'])
        summary=self.old_output/'results.json';q.write(summary,old)
        self.plan={**prior,'execution_sources':{**e.PREVIOUS_SOURCES,'ownership_diagnostic_eligibility':'a'*64}}
        paths=dict(previous_results=summary,previous_plan=previous,previous_qualification=qualification,
                   observations=self.old_output/'host/host-observations.jsonl',**e.cohort_files(self.old_output,old))
        self.plan['eligibility_amendment']=dict(kind=e.KIND,attempt_seconds=5.2,charged_seconds=6,
             reused_calls=30,new_calls=50,total_calls=80,
             files={key:dict(path=str(path),sha256=q.digest(path)) for key,path in paths.items()})
        return paths,old,prior

    def validate_bound(self):return e.validate(self.plan,self.manifest,self.plan_path)

    def replace(self,paths,key,value):
        q.write(paths[key],value)
        self.plan['eligibility_amendment']['files'][key]['sha256']=q.digest(paths[key])

    def test_exact_schema_charges_sources_and_prefix_failure_binding(self):
        paths,old,prior=self.bound();self.validate_bound()
        original=copy.deepcopy(self.plan)
        for key,value in (('charged_seconds',5),('attempt_seconds',4.9),('reused_calls',29),
                          ('new_calls',51),('total_calls',81)):
            self.plan=copy.deepcopy(original);self.plan['eligibility_amendment'][key]=value
            with self.assertRaises(ValueError):self.validate_bound()
        self.plan=copy.deepcopy(original);self.plan['execution_sources']['unrelated']='0'*64
        with self.assertRaises(ValueError):self.validate_bound()
        self.plan=copy.deepcopy(original)
        for key,value in (('status','complete'),('native_calls_executed_here',30),('failures',[]),
                          ('charged_prior_seconds',1),('elapsed_s',8.1)):
            changed=copy.deepcopy(old);changed[key]=value;self.replace(paths,'previous_results',changed)
            with self.assertRaises(ValueError):self.validate_bound()
        self.replace(paths,'previous_results',old)
        changed=copy.deepcopy(prior);changed['execution_sources']['ownership_diagnostic_transport']='0'*64
        self.replace(paths,'previous_plan',changed)
        with self.assertRaises(ValueError):self.validate_bound()

    def test_observer_and_raw_hash_drift_fail_closed(self):
        paths,old,_=self.bound();self.validate_bound()
        changed=copy.deepcopy(old);changed['host']['host']['hostname']='other'
        self.replace(paths,'previous_results',changed)
        with self.assertRaises(ValueError):self.validate_bound()
        self.replace(paths,'previous_results',old)
        paths['call-29-log'].write_text(paths['call-29-log'].read_text()+'changed')
        with self.assertRaisesRegex(ValueError,'eligibility evidence drift'):self.validate_bound()


if __name__=='__main__':unittest.main()
