"""Synthetic fixed diagnostic protocol; native execution and mathematics mocked only in runner fixture."""
import copy
import json
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

import ownership_qualification_diagnostics as d
import qualify as q
import qualify_ownership as own
import test_qualify as historical
import test_qualify_ownership as qualification_tests


def counters(family, workers, arm):
    spec=own.added_cases.specification(family)
    n,k=spec['rows'],spec['regressors']+int(spec['include_intercept'])+1
    bound=k*3*(min(64,max(1,(n+16383)//16384))+1)*8
    construction=workers==4 and spec['include_intercept']
    positive=construction and family not in own.added_cases.BOUNDARY
    result=[]
    if construction:
        result=[dict(constructed_moment_rows=n,constructed_moment_columns=k,moment_workspace_bound_bytes=bound)
                if arm=='uninjected_candidate' or not positive else dict(unused_moment_bytes=bound)]
    # Synthetic event generation is deliberately independent of the runner's
    # expected_scores helper; unit tests below pin the exact boundary numbers.
    width=spec['regressors'];generated=width+int(spec['include_intercept'])
    admitted=arm=='candidate' and n*width<=4194304
    result.append(dict(score_admitted=int(admitted),avoided_packed_elements=n*width if admitted else 0,
                       avoided_packed_bytes=8*n*width if admitted else 0,
                       retained_matrix_capacity_bytes=8*n*(generated+1) if admitted else 0,
                       generated_columns=generated,kept_columns=width))
    return result


def cell_calls(family, workers):
    rows=[]
    for arm in ('incumbent','uninjected_candidate','uninjected_candidate','candidate','candidate'):
        rows.append(dict(arm=arm,status='passed',oracle='passed',fingerprint='exact',worker_signature='same',
                         route_trace=[],route_hierarchy=[],gate_events=[],action_resources=[{'used':1}],
                         solver_trace_lines=['present'],ownership_transport={'normalized_action_resources':[{'used':1}]},
                         ownership_diagnostic={'events':counters(family,workers,arm)},cell_diagnostic={}))
    return rows


class Contract(unittest.TestCase):
    def test_fixed_eighty_bijection_budget_and_frozen_sources(self):
        value=qualification_tests.manifest()
        value['files']={a['path']:str(i)*64 for i,a in enumerate(value['artifacts'].values(),1)}
        value['files'].update({p:'f'*64 for p in value['ownership_binding'].values()})
        with tempfile.TemporaryDirectory() as tmp,patch.object(d,'verify_bound',return_value={}):
            root=Path(tmp);q.write(root/'qualification.json',value)
            plan=d.freeze(root/'qualification.json',root/'diagnostic.json')
            self.assertEqual(len(plan['schedule']),80)
            self.assertEqual((plan['native_calls'],plan['timeout_s'],plan['budget_s'],plan['retries']),(80,120,2400,0))
            self.assertEqual(len({e['cell'] for e in plan['schedule']}),16)
            d.load(root/'diagnostic.json')
            for changed in ({**plan,'budget_s':2401},{**plan,'native_calls':81},
                            {**plan,'candidate_sha256':'0'*64},{**plan,'schedule':plan['schedule'][:-1]},
                            {**plan,'schedule':[plan['schedule'][1],plan['schedule'][0],*plan['schedule'][2:]]},
                            {**plan,'execution_sources':{}}):
                q.write(root/'diagnostic.json',changed)
                with self.assertRaises(ValueError):d.load(root/'diagnostic.json')
            q.write(root/'diagnostic.json',plan)
            with self.assertRaises(FileExistsError):d.save_new(root/'diagnostic.json',{})
            q.write(root/'qualification.json',{**value,'seed':0})
            with self.assertRaises(ValueError):d.load(root/'diagnostic.json')

    def test_actual_positive_work_controls_missing_trace_and_repeat_changes(self):
        for family in own.ADDED:
            for workers in (1,4):
                calls=cell_calls(family,workers)
                evidence=d.mechanism_evidence(family,workers,calls,'ownership_both')
                self.assertEqual(evidence['moment_omission_eligible'],workers==4
                                 and family not in (*own.added_cases.BOUNDARY,'ownership-inactive-zero'))
                for armindex in (1,2,3,4):
                    bad=copy.deepcopy(calls);bad[armindex]['solver_trace_lines']=[]
                    with self.assertRaises(ValueError):d.mechanism_evidence(family,workers,bad,'ownership_both')
                bad=copy.deepcopy(calls);bad[4]['fingerprint']='changed'
                with self.assertRaises(ValueError):d.mechanism_evidence(family,workers,bad,'ownership_both')
                bad=copy.deepcopy(calls);bad[4]['ownership_transport']['normalized_action_resources']=[{'used':2}]
                with self.assertRaises(ValueError):d.mechanism_evidence(family,workers,bad,'ownership_both')
        calls=cell_calls(own.ADDED[0],4)
        for indexes in ((1,2),(3,4)):
            bad=copy.deepcopy(calls)
            for i in indexes:bad[i]['ownership_diagnostic']['events']=bad[i]['ownership_diagnostic']['events'][-1:]
            with self.assertRaises(ValueError):d.mechanism_evidence(own.ADDED[0],4,bad,'ownership_both')
        calls=cell_calls(own.ADDED[0],4)
        for i in (3,4):calls[i]['cell_diagnostic']={'bound':False}
        with self.assertRaises(ValueError):d.mechanism_evidence(own.ADDED[0],4,calls,'cells_eager_both')
        calls=cell_calls(own.ADDED[0],1)
        for i in (1,2):calls[i]['ownership_diagnostic']['events']=counters(own.ADDED[0],4,'uninjected_candidate')
        with self.assertRaises(ValueError):d.mechanism_evidence(own.ADDED[0],1,calls,'ownership_both')

    def test_deferred_mechanism_requires_binding_and_paired_commitment_evidence(self):
        self.assertEqual(d.mechanism('candidate','cells_deferred_both'),('both','deferred'))
        self.assertEqual(d.mechanism('uninjected_candidate','cells_deferred_both'),('none','none'))
        with self.assertRaises(ValueError):d.mechanism('candidate','cells_unknown_both')
        bound=dict(event='bound',attempted=0,admitted=0,refusal='None',cells=0,retained_bytes=0,peak_bytes=0)
        for outcome in ('none','admitted','refused'):
            calls=cell_calls(own.ADDED[0],4)
            events=[bound]
            if outcome!='none':
                events.append(dict(event='attempt',attempted=1,admitted=int(outcome=='admitted'),
                                   refusal='None' if outcome=='admitted' else 'Some(MemoryBudget)',
                                   cells=10 if outcome=='admitted' else 0,
                                   retained_bytes=100 if outcome=='admitted' else 0,peak_bytes=200))
            count=int(outcome!='none')
            for i in (3,4):
                calls[i]['cell_diagnostic']=dict(bound=True,events=events)
                calls[i]['deferred_commitment']=dict(installations=count,attempts=count)
            d.mechanism_evidence(own.ADDED[0],4,calls,'cells_deferred_both')
            for change in ('missing','count','binding'):
                bad=copy.deepcopy(calls)
                for i in (3,4):
                    if change=='missing':bad[i].pop('deferred_commitment')
                    elif change=='count':bad[i]['deferred_commitment']['installations']+=1
                    else:bad[i]['cell_diagnostic']['bound']=False
                with self.assertRaises(ValueError):d.mechanism_evidence(own.ADDED[0],4,bad,'cells_deferred_both')

    def test_no_intercept_two_fe_loaded_rhs_control_requires_no_moments(self):
        family='ownership-inactive-zero'
        for workers in (1,4):
            calls=cell_calls(family,workers)
            for i in (3,4):calls[i]['ownership_diagnostic']['events'].insert(0,dict(unused_moment_bytes=0))
            evidence=d.mechanism_evidence(family,workers,calls,'ownership_both')
            self.assertFalse(evidence['baseline_construction_required'])
            self.assertFalse(evidence['moment_omission_eligible'])
            self.assertEqual(evidence['expected_finalist_scores']['avoided_packed_elements'],80000)
            for indices in ((1,2),(3,4)):
                bad=copy.deepcopy(calls)
                for i in indices:
                    bad[i]['ownership_diagnostic']['events'].insert(0,dict(constructed_moment_rows=20000,
                        constructed_moment_columns=5,moment_workspace_bound_bytes=360))
                with self.assertRaises(ValueError):d.mechanism_evidence(family,workers,bad,'ownership_both')
            bad=copy.deepcopy(calls)
            for i in (3,4):bad[i]['ownership_diagnostic']['events'][0]['unused_moment_bytes']=360
            with self.assertRaises(ValueError):d.mechanism_evidence(family,workers,bad,'ownership_both')

    def test_boundary_w4_requires_preserved_construction_in_both_arms(self):
        for family in own.added_cases.BOUNDARY:
            calls=cell_calls(family,4)
            evidence=d.mechanism_evidence(family,4,calls,'ownership_both')
            self.assertTrue(evidence['baseline_construction_required'])
            self.assertFalse(evidence['moment_omission_eligible'])
            self.assertFalse(evidence['positive_omission_demonstrated'])
            for indices in ((1,2),(3,4)):
                # Both matching repeats must fail if they silently omit the
                # required work or wrongly report a positive ownership omission.
                for mode in ('missing','omitted','wrong_width'):
                    bad=copy.deepcopy(calls)
                    for index in indices:
                        events=bad[index]['ownership_diagnostic']['events']
                        if mode=='missing':events.pop(0)
                        elif mode=='omitted':events[0]={'unused_moment_bytes':events[0]['moment_workspace_bound_bytes']}
                        else:events[0]['constructed_moment_columns']-=1
                    with self.assertRaises(ValueError):d.mechanism_evidence(family,4,bad,'ownership_both')
            w1=cell_calls(family,1)
            self.assertFalse(d.mechanism_evidence(family,1,w1,'ownership_both')['baseline_construction_required'])
            for index in (1,2):w1[index]['ownership_diagnostic']['events']=counters(family,4,'uninjected_candidate')
            with self.assertRaises(ValueError):d.mechanism_evidence(family,1,w1,'ownership_both')

    def test_score_boundary_and_other_families_require_actual_eliminated_copy(self):
        boundary_bytes=(33554304,33554432,33554560)
        for family,packed_bytes in zip(own.added_cases.BOUNDARY,boundary_bytes):
            for workers in (1,4):
                calls=cell_calls(family,workers)
                evidence=d.mechanism_evidence(family,workers,calls,'ownership_both')
                self.assertEqual(evidence['packed_cache_bytes'],packed_bytes)
                admitted=packed_bytes<=33554432
                self.assertEqual(evidence['score_cache_ownership_eligible'],admitted)
                expected=evidence['expected_finalist_scores']
                self.assertEqual((expected['generated_columns'],expected['kept_columns']),(17,16))
                self.assertEqual(expected['score_admitted'],int(admitted))
                self.assertEqual(expected['avoided_packed_bytes'],packed_bytes if admitted else 0)
                self.assertEqual(expected['retained_matrix_capacity_bytes'],packed_bytes//16*18 if admitted else 0)
        for family in own.ADDED:
            for workers in (1,4):
                calls=cell_calls(family,workers)
                spec=own.added_cases.specification(family)
                for indices in ((1,2),(3,4)):
                    for field in ('score_admitted','avoided_packed_elements','avoided_packed_bytes',
                                  'retained_matrix_capacity_bytes','generated_columns','kept_columns'):
                        bad=copy.deepcopy(calls)
                        for i in indices:
                            score=bad[i]['ownership_diagnostic']['events'][-1]
                            score[field]=(1-score[field]) if field=='score_admitted' else score[field]+1
                        with self.assertRaises(ValueError):d.mechanism_evidence(family,workers,bad,'ownership_both')
                if family!='ownership-cache-above':
                    # The formerly accepted all-zero mechanism must fail even
                    # when both repeats agree and all scientific fields match.
                    bad=copy.deepcopy(calls)
                    for i in (3,4):
                        event=bad[i]['ownership_diagnostic']['events'][-1]
                        for field in ('score_admitted','avoided_packed_elements','avoided_packed_bytes','retained_matrix_capacity_bytes'):
                            event[field]=0
                    with self.assertRaises(ValueError):d.mechanism_evidence(family,workers,bad,'ownership_both')
                self.assertEqual(calls[1]['ownership_diagnostic']['events'][-1]['score_admitted'],0)
                self.assertEqual(calls[3]['ownership_diagnostic']['events'][-1]['kept_columns'],spec['regressors'])

    def test_cell_scope_requires_original_hierarchy_and_ownership_exactness(self):
        from test_cmgschur import result,bits
        raw=result();changed=copy.deepcopy(raw);changed['matrices']['RSOLVER']['bits'][2]=bits(7.)
        def snap(value,fingerprint,hierarchy):
            return value,None,dict(fingerprint=fingerprint,route_trace=[{'installed':'1'}],route_hierarchy=[hierarchy],gate_events=[],
                                   action_resources=[dict(final='0',preset='routing_two_gates_v1',iterations_used=15,
                                                          iterations_limit=100,memory_bytes_used=1000,memory_bytes_limit=2000)])
        snapshots=dict(incumbent=snap(raw,'original',{'level':1}),uninjected_candidate=snap(raw,'original',{'level':1}),
                       candidate=snap(changed,'changed',{'level':1}))
        meta=dict(categorical_intercepts=True,absorbed_terms=2)
        with patch.object(q.cmgschur,'scientific_comparison'):
            for request in own.CELL_REQUESTS:
                d.compare_cell(qualification_tests.manifest(request=request),meta,snapshots)
            with self.assertRaises(ValueError):d.compare_cell(qualification_tests.manifest(),meta,snapshots)
            snapshots['candidate'][2]['route_hierarchy']=[{'level':2}]
            for request in own.CELL_REQUESTS:
                with self.assertRaises(ValueError):d.compare_cell(qualification_tests.manifest(request=request),meta,snapshots)


class Monitor(historical.Monitor):
    def __enter__(self):
        super().__enter__();self.sample();self.started=time.monotonic_ns();self.finished=None;return self
    def sample(self):
        now=time.monotonic_ns()
        with (self.directory/'host-observations.jsonl').open('a') as stream:
            stream.write(json.dumps(dict(kind='process',monotonic_ns=now,finished_monotonic_ns=now,competing_compute=[]))+'\n')
    def __exit__(self,*_):self.finished=time.monotonic_ns();self.sample()
    def report(self):
        return dict(isolation_ok=self.finished is not None,errors=[],host={'hostname':'synthetic'},
                    started_monotonic_ns=self.started,measurement_end_monotonic_ns=self.finished)


class Runner(unittest.TestCase):
    def setUp(self):
        self.f=historical.Qualification('runTest');self.f.setUp();self.addCleanup(self.f.doCleanups)
        f=self.f;manifest=f.plan();self.manifest=manifest
        manifest['cases']=[{**manifest['cases'][0],'id':family,'family':family} for family in own.ADDED]
        manifest['artifacts']['candidate']['request']='ownership_both'
        self.plan=dict(platform='Darwin',request='ownership_both',schedule=d.schedule(qualification_tests.manifest()),
                       qualification_sha256='a'*64,candidate_sha256='b'*64,incumbent_sha256='c'*64,
                       qualification_identity='d'*64,execution_sources={},budget_s=2400)
        self.plan_path=f.root/'diagnostic.json';q.write(self.plan_path,self.plan)
        self.loader=patch.object(d,'load',return_value=(self.plan,manifest,f.manifest,{}));self.loader.start();self.addCleanup(self.loader.stop)
        self.output=f.output/'diagnostic'

    def execute(self,command,log,timeout):
        resources=self.f.execute(command,log,timeout)
        entry=self.plan['schedule'][len(self.f.calls)-1]
        with Path(command[-2]).open('rb') as stream:_,argv=q.case_header(stream)
        parent=own.argument_parent(argv)
        lines=[f'FEREG_SOLVER_TRACE|phase=routing_action|final=0|preset=routing_two_gates_v1|iterations=15|native_limit=100|memory_bytes={parent+100}|memory_limit={parent+1000}|certified=1']
        if entry['arm']=='candidate' and self.plan['request']=='cells_deferred_both':
            lines.insert(0,'FEREG_CELLS|event=bound|attempted=0|admitted=0|refusal=None|cells=0|retained_bytes=0|peak_bytes=0')
        if entry['arm']!='incumbent':
            lines += ['FEREG_OWNERSHIP|'+'|'.join(f'{k}={v}' for k,v in event.items())
                      for event in counters(entry['family'],entry['workers'],entry['arm'])]
        with log.open('a') as stream:stream.write('\n'.join(lines)+'\n')
        return resources

    def test_positive_actual_eighty_call_runner_proof_and_tamper_rejection(self):
        result=d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=Monitor)
        self.assertEqual((result['status'],result['native_calls_started'],len(result['cells'])),('complete',80,16))
        self.assertEqual(self.f.oracle.call_count,80)
        self.assertEqual(sum('reference' in c for c in result['calls']),48)
        self.assertTrue(all(c['timing_field']=='NATIVE_SECONDS' for c in result['calls']))
        proof=d.prove(self.plan_path,self.output/'results.json',self.output/'proof.json')
        self.assertEqual(proof['status'],'passed');self.assertFalse(proof['performance_claim'])
        with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'proof.json')
        saved=copy.deepcopy(result)
        for key,value in (('status','failed'),('platform','Linux'),('native_calls_started',79),
                          ('elapsed_s',2401),('candidate_sha256','0'*64)):
            altered={**result,key:value};q.write(self.output/'results.json',altered)
            with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'bad-proof.json')
            self.assertFalse((self.output/'bad-proof.json').exists())
        q.write(self.output/'results.json',saved)
        observed=self.output/'host/host-observations.jsonl';original=observed.read_text()
        events=[json.loads(line) for line in original.splitlines()]
        events[-1]['competing_compute']=[{'synthetic':'external'}]
        observed.write_text(''.join(json.dumps(e)+'\n' for e in events))
        changed={**saved,'host_observations_sha256':q.digest(observed)};q.write(self.output/'results.json',changed)
        with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'host-proof.json')
        observed.write_text(original);q.write(self.output/'results.json',saved)
        self.assertEqual(len(self.f.calls),80)
        log=self.output/result['calls'][0]['log'];log.write_text(log.read_text()+'tampered\n')
        with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'tampered-proof.json')
        self.assertFalse((self.output/'tampered-proof.json').exists())

    def test_deferred_eighty_call_runner_reparses_commitment_and_rejects_tamper(self):
        self.f.meta['absorbed_terms']=2
        self.plan['request']='cells_deferred_both'
        self.manifest['artifacts']['candidate']['request']='cells_deferred_both'
        result=d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=Monitor)
        self.assertEqual((result['status'],result['native_calls_started']),('complete',80))
        candidates=[c for c in result['calls'] if c['arm']=='candidate']
        self.assertEqual(len(candidates),32)
        self.assertTrue(all(c['cell_diagnostic']['requested_cells']=='deferred'
                            and c['deferred_commitment']==dict(installations=0,attempts=0) for c in candidates))
        proof=d.prove(self.plan_path,self.output/'results.json',self.output/'proof.json')
        self.assertEqual(proof['status'],'passed')
        candidates[0]['deferred_commitment']['installations']=1
        q.write(self.output/'results.json',result)
        with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'bad-proof.json')
        self.assertEqual(len(self.f.calls),80)

    def test_linux_proof_accepts_padded_qacct_but_requires_exact_reservation_and_marker(self):
        result=d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=Monitor)
        self.plan['platform']='Linux';result['platform']='Linux'
        result['host']['host'].update(hostname='scc-ua1',job_id='123',task_id='0',slots=28,pe='omp28')
        q.write(self.output/'results.json',result)
        account=self.output/'accounting.txt';job=self.output/'job.log'
        valid='jobnumber 123\ntaskid undefined\nhostname scc-ua1.scc.bu.edu\nproject welfgr\nfailed 0\nexit_status 0\nend_time now\nslots 28\ngranted_pe omp28\n'
        # qacct aligns values with trailing spaces (including slots); retain
        # that real framing through every positive and negative proof check.
        valid=''.join(line+'                  \n' for line in valid.splitlines())
        account.write_text(valid);job.write_text(d.MARKER+'\n')
        # Reservation/census validators have their own maintained raw witnesses.
        # This fixture specifically executes real q.accounting and marker gates.
        with patch.object(d,'verify_host',return_value=result['host']):
            for bad in (valid.replace('exit_status 0','exit_status 1'),valid.replace('123','124'),
                        valid.replace('undefined','2'),valid.replace('scc-ua1','scc-ua2'),
                        valid.replace('slots 28','slots 4'),valid.replace('omp28','mpi'),
                        valid+'jobnumber 123\n'):
                account.write_text(bad)
                with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'linux-proof.json',accounting=account,job_log=job)
            account.write_text(valid)
            for bad in ('',d.MARKER+'\n'+d.MARKER+'\n'):
                job.write_text(bad)
                with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'linux-proof.json',accounting=account,job_log=job)
            job.write_text(d.MARKER+'\n')
            proof=d.prove(self.plan_path,self.output/'results.json',self.output/'linux-proof.json',accounting=account,job_log=job)
            self.assertEqual(proof['accounting_sha256'],q.digest(account))

    def test_failed_invocation_keeps_prefix_and_partial_files_without_retry(self):
        def failed(command,log,timeout):
            value=self.execute(command,log,timeout)
            if len(self.f.calls)==2:raise RuntimeError('synthetic native failure')
            return value
        with self.assertRaises(RuntimeError):d.run(self.plan_path,self.output,execute=failed,monitor_factory=Monitor)
        result=q.read(self.output/'results.json')
        self.assertEqual((result['status'],result['native_calls_started'],len(self.f.calls)),('failed',2,2))
        self.assertEqual([c['status'] for c in result['calls']],['passed','failed'])
        self.assertTrue((self.output/result['calls'][1]['target']).exists())
        self.assertIn('result_sha256',result['calls'][1])
        with self.assertRaises(ValueError):d.prove(self.plan_path,self.output/'results.json',self.output/'proof.json')
        with self.assertRaises(ValueError):d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=Monitor)
        self.assertEqual(len(self.f.calls),2)

    def test_binding_failure_starts_zero_native_calls(self):
        self.loader.stop()
        with patch.object(d,'load',side_effect=ValueError('changed artifact')):
            with self.assertRaises(ValueError):d.run(self.plan_path,self.output,execute=self.execute,monitor_factory=Monitor)
        result=q.read(self.output/'results.json')
        self.assertEqual((result['status'],result['native_calls_started'],self.f.calls),('failed',0,[]))


if __name__=='__main__':unittest.main()
