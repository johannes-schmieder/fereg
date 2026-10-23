"""Bounded synthetic development-runner tests; never execute a native plugin."""
import copy
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownership_cells as o
import qualify as q
from test_cmgschur import result, case_bytes, bits


class Monitor:
    def __init__(self, output, workers):
        self.output = output
        self.workers = workers
    def __enter__(self):
        self.output.mkdir()
        (self.output / 'host-observations.jsonl').write_text('{"kind":"synthetic"}\n')
        return self
    def __exit__(self, *_): pass
    def check(self): pass
    def report(self):
        return {'isolation_ok': True, 'errors': [], 'host': {'hostname': 'synthetic'}, 'synthetic': True}


def bare_plan(panel=1, ownership='none'):
    arms = {k: list(v) for k, v in o.mechanisms(panel, ownership).items()}
    plan = dict(schema=1, kind=o.KIND, platform='Darwin', panel=panel, ownership=ownership,
                data_seed=o.DATA_SEED, order_seed=o.ORDER_SEEDS[panel], threads=[1, 4], blocks=6, warmups=1,
                timeout_s=120, budget_s=3600, diagnostic_budget_s=600, diagnostic_repeats=3,
                observer='desktop', arms=arms, aliases=o.aliases(arms), conditional_authorization=panel==3,
                panel2_result='previous.json' if panel==3 else None,
                cases=[dict(id=f, family=f, input=f+'.case', metadata=f+'.json',
                            oracle={'kind':'routing','path':f+'.oracle.json'}, memory_limit_bytes=None) for f in o.FAMILIES])
    plan.update(schedule=o.schedule(plan), diagnostic_schedule=o.schedule(plan, True))
    plan['call_cap'] = len(plan['schedule'])
    return plan


class Contracts(unittest.TestCase):
    def test_cell_evidence_retains_skip_refusal_and_single_reused_build(self):
        bound = 'FEREG_CELLS|event=bound|attempted=0|admitted=0|refusal=None|cells=0|retained_bytes=0|peak_bytes=0'
        built = 'FEREG_CELLS|event=attempt|attempted=1|admitted=1|refusal=None|cells=5|retained_bytes=160|peak_bytes=500'
        log = '\n'.join([bound, built, built.replace('event=attempt', 'event=reuse')])
        self.assertTrue(o.cell_diagnostic(log, ('both', 'eager'))['constructed'])
        self.assertFalse(o.cell_diagnostic(bound, ('none', 'eager'))['constructed'])
        self.assertFalse(o.cell_diagnostic('', ('none', 'none'))['bound'])
        refused = built.replace('admitted=1', 'admitted=0').replace('refusal=None', 'refusal=Some(MemoryBudget)').replace('cells=5', 'cells=0').replace('retained_bytes=160', 'retained_bytes=0')
        self.assertFalse(o.cell_diagnostic(bound+'\n'+refused, ('none', 'eager'))['constructed'])
        self.assertFalse(o.cell_diagnostic(bound+'\n'+refused+'\n'+refused.replace('event=attempt', 'event=reuse'), ('none', 'eager'))['constructed'])
        for bad, mechanism in [(log, ('none', 'none')), (built, ('none', 'eager')),
                               (log+'\n'+built, ('none', 'eager')),
                               (bound+'\n'+refused.replace('Some(MemoryBudget)', 'None'), ('none', 'eager')),
                               (log.replace('peak_bytes=500', 'peak_bytes=159'), ('none', 'eager')),
                               (log.replace('cells=5', 'cells=-1'), ('none', 'eager'))]:
            with self.assertRaises(ValueError): o.cell_diagnostic(bad, mechanism)

    def test_actual_ownership_phase_counters_fail_closed(self):
        score='FEREG_OWNERSHIP|score_admitted=1|avoided_packed_elements=100|avoided_packed_bytes=800|retained_matrix_capacity_bytes=1600|generated_columns=3|kept_columns=2'
        log='FEREG_OWNERSHIP|unused_moment_bytes=64\n'+score
        self.assertEqual(len(o.ownership_diagnostic(log,('both','none'))['events']),2)
        for bad,mechanism in [(log,('none','none')), (score.replace('800','801'),('scores','none')),
                              (score+'\n'+score,('scores','none')),('',('both','none')),
                              (score.replace('score_admitted=1','score_admitted=0'),('scores','none'))]:
            with self.assertRaises(ValueError):o.ownership_diagnostic(bad,mechanism)
        refused=score.replace('score_admitted=1','score_admitted=0').replace('=100|','=0|').replace('=800|','=0|').replace('=1600|','=0|')
        self.assertEqual(o.ownership_diagnostic(refused,('scores','none'))['events'][0]['score_admitted'],0)

    def test_fixed_schedules_mirrors_and_prospective_aliases(self):
        for panel, ownership in [(1,'none'),(2,'both'),(3,'scores')]:
            plan = bare_plan(panel, ownership)
            o.validate(plan)
            self.assertEqual(plan['call_cap'], 1248)
            self.assertEqual(len(plan['diagnostic_schedule']), 288)
            self.assertEqual(plan['schedule'], o.schedule(plan))
            for block in range(6):
                for family in o.FAMILIES:
                    for workers in (1,4):
                        rows = [r for r in plan['schedule'] if r['cell']==f'{family}-t{workers}' and r['block']==block]
                        self.assertEqual([r['arm'] for r in rows[:4]], [r['arm'] for r in reversed(rows[4:])])
        plan = bare_plan(2,'none')
        self.assertEqual(plan['aliases'], {'B':'B','O':'B','S':'S','O+S':'S'})
        self.assertEqual(plan['call_cap'], 624)
        self.assertEqual(len(plan['diagnostic_schedule']), 144)
        for key,value in [('budget_s',3601),('blocks',7),('timeout_s',121),('order_seed',42),('diagnostic_repeats',4)]:
            altered=copy.deepcopy(plan);altered[key]=value
            with self.assertRaises(ValueError):o.validate(altered)
        plan=bare_plan(3,'both');plan['conditional_authorization']=False
        with self.assertRaises(ValueError):o.validate(plan)

    def test_objective_uses_arithmetic_differences_with_equal_settings(self):
        plan=bare_plan()
        calls=[]
        for entry in plan['schedule']:
            # Relative ratios favor the opposite arm of the absolute-time objective.
            large=entry['cell']=='weak-t1'
            base=10.0 if large else .01
            delta=(-1. if large else .001) if entry['arm']=='moments' else (-.001 if not large else 0.) if entry['arm']=='scores' else 0.
            calls.append({**entry,'status':'passed','seconds':base+delta})
        comparisons, objectives=o.summaries(plan,calls)
        self.assertAlmostEqual(objectives['moments'],(-1.+23*.001)/24)
        self.assertAlmostEqual(objectives['scores'],-23*.001/24)
        self.assertLess(objectives['moments'],objectives['scores'])
        self.assertGreater(np.mean([r['mean_ratio'] for r in comparisons if r['arm']=='moments']),
                           np.mean([r['mean_ratio'] for r in comparisons if r['arm']=='scores']))
        self.assertEqual(len(comparisons),72)
        with self.assertRaises(ValueError):o.summaries(plan,calls[:-1])
        bad=copy.deepcopy(calls);bad[-1]['block']=0
        with self.assertRaises(ValueError):o.summaries(plan,bad)

    def test_request_only_changes_execution_options_and_one_token(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/'input.case';source.write_bytes(case_bytes())
            def inspect(path):
                with Path(path).open('rb') as stream:q.case_header(stream);return {'body_sha256':hashlib.sha256(stream.read()).hexdigest()}
            with patch.object(o.routing_cases,'inspect_case',side_effect=inspect):
                for i,mechanism in enumerate([('none','none'),('both','none'),('scores','deferred')]):
                    output=root/f'{i}.case';o.prepare_request(source,output,mechanism,4)
                    with output.open('rb') as stream:
                        shape,args=q.case_header(stream);body=stream.read()
                    self.assertEqual(body,b'body\n9\nthreads=1\n')
                    self.assertIn(b'threads=4',args)
                    self.assertEqual([a for a in args if a.startswith(b'inject=')],
                                     [] if i==0 else [('inject='+o.token(mechanism)).encode()])
                source.write_bytes(case_bytes([b'estimate-ols',b'threads=1',b'inject=other']))
                with self.assertRaises(ValueError):o.prepare_request(source,root/'bad.case',('none','none'),1)

    def test_historical_shapes_use_new_deterministic_streams(self):
        for family in o.FAMILIES:
            a,w,intercept=o.columns(family);b,v,other=o.columns(family)
            self.assertTrue(all(np.array_equal(x,y) for x,y in zip(a,b)))
            self.assertLessEqual(len(a[0]),100000)
            self.assertEqual(intercept,other)
            self.assertEqual(w is None,v is None)
        a,_,_=o.columns('repeated');self.assertTrue(np.array_equal(a[0][::4],a[0][1::4]))
        a,w,_=o.columns('weight-dynamic');self.assertGreater(float(w.max()/w.min()),1000000)
        a,_,intercept=o.columns('inactive-zero');self.assertFalse(intercept)
        self.assertTrue(np.array_equal(a[0][::2],-a[0][1::2]))

    def test_freeze_binds_actual_inputs_sources_reference_and_control_proof(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            items=[]; files={}; observed={}
            for family in o.FAMILIES:
                source=root/(family+'.case');source.write_text(family)
                body=hashlib.sha256(family.encode()).hexdigest()
                meta=dict(seed=o.DATA_SEED,derived_seed=o.DATA_SEED+o.FAMILIES.index(family),
                          generator_sha256='f'*64,generator_sources={'generator':'e'*64},
                          case_sha256=q.digest(source),body_sha256=body)
                observed[source.name]={'body_sha256':body,'absorbed_terms':2,'categorical_intercepts':True}
                q.write(root/(family+'.json'),meta);q.write(root/(family+'.oracle.json'),{})
                for name in (family+'.case',family+'.json',family+'.oracle.json'):files[name]=q.digest(root/name)
                items.append(dict(id=family,family=family,input=family+'.case',metadata=family+'.json',
                                  oracle={'kind':'routing','path':family+'.oracle.json'},memory_limit_bytes=None))
            q.write(root/'panel.json',dict(kind=o.KIND+'-inputs',seed=o.DATA_SEED,families=list(o.FAMILIES),
                                         generator_sha256='f'*64,generator_sources={'generator':'e'*64},cases=items,files=files))
            for name in ('candidate','reference','harness'):(root/name).write_text(name)
            q.write(root/'candidate-source.json',{'fereg':'a'*40})
            q.write(root/'reference-source.json',{'fereg':o.REFERENCE_REVISION})
            proof=dict(status='passed',unmodified_controls_exact=True,candidate_sha256=q.digest(root/'candidate'),
                       reference_sha256=q.digest(root/'reference'),inputs_sha256=q.digest(root/'panel.json'),
                       kind=o.KIND+'-controls',platform='Darwin',native_calls_started=48,failures=[],retries=0,elapsed_s=1,
                       schedule=o.controls_schedule(),calls=[{**row,'status':'passed','oracle':'passed'} for row in o.controls_schedule()],
                       plan={'execution_sources':{name:q.digest(path) for name,path in o.sources().items()}})
            q.write(root/'proof.json',proof)
            cfg=dict(platform='Darwin',panel=1,ownership='none',inputs='panel.json',harness='harness',
                     candidate=dict(path='candidate',revision='a'*40,source_manifest='candidate-source.json',build='synthetic'),
                     reference=dict(path='reference',revision=o.REFERENCE_REVISION,source_manifest='reference-source.json',build='synthetic'),
                     control_proof='proof.json',diagnostic_repeats=3,conditional_authorization=False,panel2_result=None)
            q.write(root/'config.json',cfg)
            with patch.object(o.routing_cases,'inspect_case',side_effect=lambda p,*_:observed[Path(p).name]), \
                 patch.object(o.routing_oracle,'load_reference',side_effect=lambda p,*_:({}, {p:q.digest(p)})):
                plan=o.freeze(root/'config.json',root/'plan.json')
                o.verify_files(q.read(root/'plan.json'),root/'plan.json')
                self.assertEqual(plan['call_cap'],1248)
                proof['unmodified_controls_exact']=False;q.write(root/'proof.json',proof)
                with self.assertRaisesRegex(ValueError,'control proof'):o.freeze(root/'config.json',root/'bad-plan.json')
                proof['unmodified_controls_exact']=True;q.write(root/'proof.json',proof)
                (root/'weak.case').write_text('mutated body')
                with self.assertRaisesRegex(ValueError,'generated input changed'):o.freeze(root/'config.json',root/'bad-input-plan.json')


class Runner(unittest.TestCase):
    def setUp(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup);self.root=Path(temp.name)
        self.plan=bare_plan();self.manifest=self.root/'plan.json';self.output=self.root/'run'
        self.plugin=self.root/'plugin';self.plugin.write_bytes(b'fake plugin')
        (self.root/'harness').write_bytes(b'fake harness')
        self.meta={'rows':2,'columns':6,'saved':[],'outputs':dict(zip(('sample','residual','drop_reason','xb','xbd','absorbed'),range(6)))}
        self.data=np.array([[1.,1.],[.2,-.2],[0.,0.],[.3,.4],[.5,.2],[.2,-.2]])
        self.files={'plugin':q.digest(self.plugin),'harness':q.digest(self.root/'harness')}
        for item in self.plan['cases']:
            path=self.root/item['input'];path.write_bytes(case_bytes());self.files[item['input']]=q.digest(path)
        self.plan.update(artifacts={'candidate':{'path':'plugin'}},harness='harness',files=self.files,
                         execution_sources={n:q.digest(p) for n,p in o.sources().items()},inputs='weak.case')
        q.write(self.manifest,self.plan)
        self.calls=[];self.started=[];self.real_write=q.write
        self.addCleanup(patch.stopall)
        patch.object(o.routing_cases,'scratch',side_effect=lambda p:Path(p)).start()
        patch.object(o.platform,'system',return_value='Darwin').start()
        patch.object(q,'metadata',return_value=self.meta).start()
        self.oracle=patch.object(q.science,'check_oracle').start()
        self.comparison=patch.object(q.cmgschur,'scientific_comparison').start()
        def inspect(path,*_):
            with Path(path).open('rb') as stream:q.case_header(stream);return {'body_sha256':hashlib.sha256(stream.read()).hexdigest()}
        patch.object(o.routing_cases,'inspect_case',side_effect=inspect).start()
        # Avoid O(n²) synthetic output I/O; preserve each started-call census and
        # terminal result. All actual raw heads/logs/arrays still use real I/O.
        def write(path,value):
            if Path(path).name=='results.json' and value['status']=='running':
                self.started.append((value['native_calls_started'],value['calls'][-1]['status'] if value['calls'] else None))
            else:self.real_write(path,value)
        patch.object(q,'write',side_effect=write).start()

    def execute(self,command,log,timeout):
        self.calls.append(command)
        self.assertLessEqual(timeout,120)
        target=Path(command[-1]);self.real_write(target,{**result(),'columns':[]})
        self.data.tofile(target.with_suffix('.json.columns'))
        log.write_text('NATIVE_SECONDS=0.010000\n')
        return {'maxrss_bytes':1024}

    def controls_config(self):
        config=self.root/'controls.json'
        self.real_write(config,dict(platform='Darwin',inputs='weak.case',harness='harness',candidate={},reference={}))
        reference=self.root/'reference-plugin';reference.write_bytes(b'accepted plugin')
        def bind_inputs(cfg,config_path,output):
            files={str(self.root/name):digest for name,digest in self.files.items()}
            def bind(name):
                path=(self.root/name).resolve();files[str(path)]=q.digest(path);return str(path)
            items=copy.deepcopy(self.plan['cases'])
            for item in items:item['input']=str(self.root/item['input'])
            artifacts={'candidate':{'path':bind('plugin')},'reference':{'path':bind('reference-plugin')}}
            return files,bind,bind('weak.case'),items,artifacts
        patch.object(o,'bind_inputs_artifacts',side_effect=bind_inputs).start()
        return config

    def test_controls_exact_48_calls_and_success_only_proof(self):
        config=self.controls_config()
        def execute(command,log,timeout):
            plan=q.read(self.output/'control-plan.json')
            self.assertEqual(len(plan['schedule']),48)
            self.assertEqual(plan['budget_s'],600)
            self.assertNotIn(b'inject=',Path(command[-2]).read_bytes())
            return self.execute(command,log,timeout)
        completed=o.controls(config,self.output,execute=execute)
        self.assertEqual(len(self.calls),48);self.assertEqual(self.oracle.call_count,48)
        self.assertEqual(completed['status'],'passed');self.assertTrue(completed['unmodified_controls_exact'])
        self.assertEqual(q.read(self.output/'control-proof.json'),completed)
        self.assertEqual(len(list(self.output.rglob('*.npz'))),48)
        self.assertEqual(completed['calls'],q.read(self.output/'results.json')['calls'])
        self.assertFalse(completed['performance_claim'])
        with self.assertRaisesRegex(ValueError,'no retries'):o.controls(config,self.output,execute=execute)

    def test_controls_checks_last_pair_and_preserves_failure_without_proof(self):
        config=self.controls_config()
        def changed(command,log,timeout):
            value=self.execute(command,log,timeout)
            if len(self.calls)==48:
                target=Path(command[-1]);raw=q.read(target);raw['matrices']['RB']['bits'][0]=bits(2.);self.real_write(target,raw)
            return value
        with self.assertRaisesRegex(ValueError,'uninjected exact control mismatch'):
            o.controls(config,self.output,execute=changed)
        failed=q.read(self.output/'results.json')
        self.assertEqual(len(self.calls),48);self.assertEqual(failed['native_calls_started'],48)
        self.assertEqual(failed['status'],'failed');self.assertFalse((self.output/'control-proof.json').exists())
        self.assertTrue(failed['calls'][-1]['log_sha256'])

    def test_controls_refuse_drift_and_independent_failure_without_retry(self):
        config=self.controls_config()
        def changed(command,log,timeout):
            value=self.execute(command,log,timeout);self.plugin.write_bytes(b'changed');return value
        with self.assertRaisesRegex(ValueError,'control source/input changed'):
            o.controls(config,self.output,execute=changed)
        self.assertEqual(len(self.calls),1);self.assertFalse((self.output/'control-proof.json').exists())
        self.plugin.write_bytes(b'fake plugin');self.calls=[]
        self.oracle.side_effect=ValueError('independent certificate failed')
        with self.assertRaisesRegex(ValueError,'independent certificate failed'):
            o.controls(config,self.root/'controls-failed',execute=self.execute)
        self.assertEqual(len(self.calls),1)
        failed=q.read(self.root/'controls-failed/results.json')
        self.assertEqual(failed['status'],'failed');self.assertEqual(failed['native_calls_started'],1)

    def test_full_schedule_true_timer_all_calls_and_report(self):
        completed=o.run(self.manifest,self.output,execute=self.execute,monitor_factory=Monitor)
        self.assertEqual(len(self.calls),1248)
        self.assertEqual(self.oracle.call_count,1248)
        self.assertEqual(len(completed['cells']),24)
        self.assertTrue(all(c['timing_field']=='NATIVE_SECONDS' and c['seconds']==.01 for c in completed['calls']))
        self.assertTrue(all((n,'started') in self.started for n in range(1,1249)))
        self.assertEqual(len(list(self.output.rglob('*.npz'))),96)
        self.assertFalse(list(self.output.rglob('*.columns')))
        report=o.report(self.manifest,self.output,self.root/'report.json')
        self.assertFalse(report['activation_qualified'])
        self.assertEqual(report['equal_weight_mean_difference_s'],{'moments':0.,'scores':0.,'both':0.})
        self.assertEqual(len(report['comparisons']),72)
        with self.assertRaises(ValueError):o.run(self.manifest,self.output,execute=self.execute,monitor_factory=Monitor)
        self.assertEqual(len(self.calls),1248)

    def test_native_failure_is_retained_without_retry(self):
        def fails(command,log,timeout):
            self.calls.append(command);log.write_text('retained native failure\n');raise ValueError('synthetic native failure')
        with self.assertRaisesRegex(ValueError,'synthetic native failure'):
            o.run(self.manifest,self.output,execute=fails,monitor_factory=Monitor)
        failed=q.read(self.output/'results.json')
        self.assertEqual(len(self.calls),1);self.assertEqual(failed['native_calls_started'],1)
        self.assertEqual(failed['status'],'failed');self.assertEqual(failed['calls'][0]['status'],'started')
        self.assertEqual(failed['retries'],0)

    def test_independent_science_failure_stops_and_cannot_enable_diagnostics(self):
        self.oracle.side_effect=ValueError('independent mathematics mismatch')
        with self.assertRaisesRegex(ValueError,'independent mathematics mismatch'):
            o.run(self.manifest,self.output,execute=self.execute,monitor_factory=Monitor)
        failed=q.read(self.output/'results.json')
        self.assertEqual(failed['status'],'failed');self.assertEqual(len(self.calls),1)
        with self.assertRaisesRegex(ValueError,'invalid scored diagnostic reference'):
            o.run(self.manifest,self.root/'diagnostics',diagnostic=True,scored_result=self.output/'results.json',
                  execute=self.execute,monitor_factory=Monitor)
        self.assertEqual(len(self.calls),1)

    def test_cross_worker_arrays_are_compared(self):
        def changed(command,log,timeout):
            value=self.execute(command,log,timeout)
            if len(self.calls)==5:
                values=self.data.copy();values[1,0]+=1.
                values.tofile(Path(command[-1]).with_suffix('.json.columns'))
            return value
        with self.assertRaisesRegex(ValueError,'cross-host'):
            o.run(self.manifest,self.output,execute=changed,monitor_factory=Monitor)
        self.assertEqual(len(self.calls),5)

    def test_input_drift_and_fixed_worker_change_stop(self):
        def mutate(command,log,timeout):
            value=self.execute(command,log,timeout);self.plugin.write_bytes(b'changed');return value
        with self.assertRaisesRegex(ValueError,'changed during run'):
            o.run(self.manifest,self.output,execute=mutate,monitor_factory=Monitor)
        self.assertEqual(len(self.calls),1)
        self.plugin.write_bytes(b'fake plugin');self.calls=[]
        output=self.root/'run2'
        def differs(command,log,timeout):
            value=self.execute(command,log,timeout)
            if len(self.calls)==97:
                target=Path(command[-1]);raw=q.read(target);raw['matrices']['RB']['bits'][0]=bits(2.);self.real_write(target,raw)
            return value
        with self.assertRaisesRegex(ValueError,'fixed-worker repeat changed'):
            o.run(self.manifest,output,execute=differs,monitor_factory=Monitor)
        self.assertEqual(len(self.calls),97)

    def test_wrong_source_fails_before_any_native_call(self):
        self.plan['execution_sources']['ownership_cells']='0'*64;self.real_write(self.manifest,self.plan)
        with self.assertRaisesRegex(ValueError,'execution source changed'):
            o.run(self.manifest,self.output,execute=self.execute,monitor_factory=Monitor)
        self.assertFalse(self.calls)
        self.assertEqual(q.read(self.output/'results.json')['native_calls_started'],0)

    def test_scoped_cells_still_require_exact_ownership_and_hierarchy(self):
        plan=bare_plan(2,'both');raw=result();path=self.root/'columns';self.data.tofile(path)
        def snapshot(fingerprint,hierarchy):
            return raw,path,{'fingerprint':fingerprint,'route_trace':[{'installed':'1'}], 'route_hierarchy':[hierarchy]}
        snapshots={'B':snapshot('base',{'graph':'same'}),'O':snapshot('base',{'graph':'same'}),
                   'S':snapshot('cells',{'graph':'same'}),'O+S':snapshot('cells',{'graph':'same'})}
        o.compare_warmups(plan,snapshots,self.meta)
        snapshots['O+S']=snapshot('different',{'graph':'same'})
        with self.assertRaisesRegex(ValueError,'ownership changed exact'):o.compare_warmups(plan,snapshots,self.meta)
        snapshots['O+S']=snapshot('cells',{'graph':'different'})
        with self.assertRaisesRegex(ValueError,'graph hierarchy changed'):o.compare_warmups(plan,snapshots,self.meta)


if __name__=='__main__':unittest.main()
