"""Bounded synthetic coverage-panel tests; the executor never invokes a plugin."""
import copy
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownership_cells as oc
import ownership_moments as moments
import qualify as q
import test_ownership_cells as legacy
import ownership_moments_cases as cases
import test_ownership_moments_cases as case_fixtures


def plan_fixture():
    plan = legacy.bare_plan()
    plan.update(kind=moments.KIND, settings=moments.settings(), threads=[1,2,4],
                order_seed=moments.ORDER_SEED, uncertainty_seed=moments.UNCERTAINTY_SEED,
                bootstrap_draws=4096, budget_s=2400, diagnostic_budget_s=1200,
                diagnostic_repeats=2, preflight_output='moments-preflight',
                control_expectations={
                    'weak-robust-t1':'no construction or omission at W1',
                    'weak-low-t4':'preserve baseline construction state; no omission or score ownership',
                    'weak-iid-t4':'preserve positive needed construction; no omission or score ownership',
                    'weighted-robust-t4':'no construction, omission or score ownership'},
                cases=[dict(id=f,family=f,input=f+'.case',metadata=f+'.json',
                            oracle=dict(kind='routing',path=f+'.oracle.json'),memory_limit_bytes=None)
                       for f in moments.CASE_IDS],
                expected_moments={f:dict(constructed_moment_rows=2,constructed_moment_columns=3,
                                        moment_workspace_bound_bytes=144) for f in moments.CASE_IDS})
    plan.update(schedule=moments.schedule(plan), diagnostic_schedule=moments.schedule(plan,True), call_cap=624)
    return plan


def events(plan, setting, arm):
    eligible = setting['eligible']
    built = ((eligible and arm in ('B','scores')) or setting['family'] in ('weak-iid','weak-low'))
    score = arm in ('scores','both') and (eligible or setting['workers']==1)
    result = [copy.deepcopy(plan['expected_moments'][setting['family']])] if built else []
    if eligible and arm in ('moments','both'):
        result.append(dict(unused_moment_bytes=144))
    result.append(dict(score_admitted=int(score),avoided_packed_elements=2 if score else 0,
                       avoided_packed_bytes=16 if score else 0,retained_matrix_capacity_bytes=48 if score else 0,
                       generated_columns=3,kept_columns=2))
    return result


def transport_bytes(plan, arm):
    token = None if arm == 'reference' else oc.token(plan['arms'][arm])
    return 0 if token is None else 24 + len(('inject=' + token).encode())


def evidence_fixture(plan):
    by_cell = {s['cell']:s for s in moments.settings()}
    calls = []
    for entry in plan['diagnostic_schedule']:
        call = dict(**entry,status='passed',oracle='passed',fingerprint=entry['cell'],worker_signature='same',
                    action_resources=[dict(memory_bytes_used=100+transport_bytes(plan,entry['arm']),memory_bytes_limit=1000)],
                    route_trace=[],route_hierarchy=[],solver_trace_lines=[])
        if entry['arm'] != 'reference':
            call['ownership_diagnostic'] = dict(events=events(plan,by_cell[entry['cell']],entry['arm']))
        calls.append(call)
    return dict(calls=calls)


class CoverageContracts(unittest.TestCase):
    def test_frozen_108_preflight_and_624_balanced_timing_schedule(self):
        plan = plan_fixture(); moments.validate(plan)
        self.assertEqual(len(plan['diagnostic_schedule']),108)
        self.assertEqual(len(plan['schedule']),624)
        for setting in moments.settings():
            pre = [c for c in plan['diagnostic_schedule'] if c['cell']==setting['cell']]
            self.assertEqual(pre[0]['arm'],'reference')
            self.assertEqual([c['arm'] for c in pre[1:5]], [c['arm'] for c in pre[5:][::-1]])
            for block in range(6):
                rows = [c for c in plan['schedule'] if c['cell']==setting['cell'] and c['block']==block]
                self.assertEqual(len(rows),8)
                self.assertEqual([c['arm'] for c in rows[:4]], [c['arm'] for c in rows[4:][::-1]])
        for field,value in [('budget_s',2401),('diagnostic_budget_s',1201),('diagnostic_repeats',3),
                            ('order_seed',1),('bootstrap_draws',4097),('call_cap',625)]:
            altered = copy.deepcopy(plan); altered[field]=value
            with self.subTest(field=field), self.assertRaises(ValueError):
                moments.validate(altered)

    def test_execution_requires_actual_construction_positive_omission_and_all_controls(self):
        plan = plan_fixture(); original = evidence_fixture(plan)
        checked = moments.execution_evidence(plan,original)
        self.assertEqual((checked['eligible_settings'],checked['control_settings']),(8,4))
        def mutate(cell,arm,change,repeat=0):
            changed = copy.deepcopy(original)
            call = next(c for c in changed['calls'] if c['cell']==cell and c['arm']==arm and c['block']==repeat)
            change(call)
            with self.assertRaises((ValueError,StopIteration)):
                moments.execution_evidence(plan,changed)
        mutate('weak-robust-t2','B',lambda c:c['ownership_diagnostic']['events'].pop(0))
        mutate('weak-robust-t2','scores',lambda c:c['ownership_diagnostic']['events'][0].update(constructed_moment_rows=3))
        mutate('weak-robust-t2','moments',lambda c:c['ownership_diagnostic']['events'][0].update(unused_moment_bytes=0))
        mutate('weak-robust-t2','both',lambda c:c['ownership_diagnostic']['events'][0].update(unused_moment_bytes=143))
        mutate('weak-robust-t2','scores',lambda c:c['ownership_diagnostic']['events'][-1].update(score_admitted=0))
        mutate('weak-robust-t1','B',lambda c:c['ownership_diagnostic']['events'].insert(0,plan['expected_moments']['weak-robust']))
        mutate('weak-iid-t4','B',lambda c:c['ownership_diagnostic']['events'].pop(0))
        mutate('weak-low-t4','moments',lambda c:c['ownership_diagnostic']['events'].pop(0))
        mutate('weighted-robust-t4','both',lambda c:c['ownership_diagnostic']['events'][-1].update(score_admitted=1))
        mutate('weighted-robust-t4','moments',lambda c:c['ownership_diagnostic']['events'].insert(0,dict(unused_moment_bytes=1)))
        mutate('weak-robust-t2','moments',lambda c:c.update(fingerprint='changed'))
        mutate('weak-robust-t2','moments',lambda c:c['ownership_diagnostic']['events'][-1].update(kept_columns=1),repeat=1)
        changed = copy.deepcopy(original); changed['calls'].pop()
        with self.assertRaises(ValueError): moments.execution_evidence(plan,changed)

    def test_second_repeat_route_and_accounted_memory_must_also_match(self):
        plan=plan_fixture(); original=evidence_fixture(plan)
        for field in ('action_resources','route_trace','route_hierarchy','solver_trace_lines'):
            changed=copy.deepcopy(original)
            call=next(c for c in changed['calls'] if c['cell']=='weak-robust-t2' and c['arm']=='moments' and c['block']==1)
            call[field] = ([dict(memory_bytes_used=101+transport_bytes(plan,'moments'),memory_bytes_limit=1000)]
                           if field=='action_resources' else
                           ['FEREG_SOLVER_TRACE|phase=two_fe_gates|decision=Build']
                           if field=='solver_trace_lines' else [dict(changed=True)])
            with self.subTest(field=field), self.assertRaises(ValueError):
                moments.execution_evidence(plan,changed)

    def test_reference_routes_missing_actions_and_transport_byte_mismatch_fail(self):
        plan=plan_fixture(); original=evidence_fixture(plan)
        for mutation in ['route','resource','missing-actions','missing-trace']:
            changed=copy.deepcopy(original)
            for call in changed['calls']:
                if call['cell']!='weak-robust-t2' or call['arm']!='moments': continue
                if mutation=='route':
                    call['solver_trace_lines']=['FEREG_SOLVER_TRACE|phase=two_fe_gates|decision=Build']
                elif mutation=='resource':
                    call['action_resources'][0]['memory_bytes_used']+=1
                elif mutation=='missing-actions':call['action_resources']=[]
                else:del call['solver_trace_lines']
            with self.subTest(mutation=mutation),self.assertRaises((ValueError,KeyError)):
                moments.execution_evidence(plan,changed)
        # Observed caps may differ while each original action still fits its cap.
        valid=copy.deepcopy(original)
        for i,call in enumerate(valid['calls']):call['action_resources'][0]['memory_bytes_limit']+=i
        self.assertEqual(moments.execution_evidence(plan,valid)['status'],'passed')

    def test_block_bootstrap_preserves_adverse_setting_and_shared_pairing(self):
        plan=plan_fixture(); calls=[]
        # Both mirror observations receive the same block shock: the independent
        # uncertainty unit is the block, not 12 individual arm observations.
        shocks = [-.003,-.002,-.001,.001,.002,.003]
        for entry in plan['schedule']:
            delta=shocks[entry['block']] if entry['phase']=='scored' and entry['arm']!='B' else 0
            if entry['cell']=='weak-iid-t4' and entry['arm']=='both': delta+=.004
            calls.append(dict(**entry,status='passed',seconds=.02+delta))
        comparisons,_=oc.summaries(plan,calls)
        uncertainty=moments.uncertainty(plan,comparisons)
        self.assertFalse(uncertainty['discovery_nonregression'])
        self.assertEqual(len(uncertainty['uncertainty']),36)
        self.assertEqual(uncertainty,moments.uncertainty(plan,comparisons))
        row=next(r for r in uncertainty['uncertainty'] if r['cell']=='weak-robust-t1' and r['arm']=='scores')
        # Enumerate the 4096 fixed draw-index vectors independently, retaining
        # exact block covariance and the declared percentile convention.
        rng=np.random.default_rng(2026106308)
        means=[sum(shocks[int(i)] for i in rng.integers(0,6,6))/6 for _ in range(4096)]
        np.testing.assert_allclose(row['difference_s_bootstrap_95'],np.quantile(means,[.025,.975]),rtol=0,atol=1e-17)
        bad=next(r for r in comparisons if r['cell']=='weak-iid-t4' and r['arm']=='both')
        self.assertGreater(bad['mean_excess_s'],0)


class CoverageFreeze(unittest.TestCase):
    def test_real_input_freeze_rejects_changed_hashes_and_mislabeled_request_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve(); original=root/'original'; original.mkdir()
            entries=[]; files={}
            for family in ['weak','wide','weight-dynamic']:
                path=original/(family+'.case')
                case_fixtures.source_case(path,weighted=family=='weight-dynamic')
                entries.append(dict(id=family,input=path.name));files[path.name]=q.digest(path)
            q.write(original/'panel.json',dict(kind=oc.KIND+'-inputs',seed=oc.DATA_SEED,cases=entries,files=files))
            with patch.object(oc.routing_cases,'scratch',side_effect=lambda p:Path(p).resolve()):
                panel=cases.prepare_inputs(original,root/'inputs')
                for name in ['candidate','reference','harness']:(root/name).write_bytes(name.encode())
                q.write(root/'candidate-source.json',dict(fereg='a'*40))
                q.write(root/'reference-source.json',dict(fereg=oc.REFERENCE_REVISION))
                config=dict(platform='Darwin',inputs='inputs/panel.json',harness='harness',
                            candidate=dict(path='candidate',revision='a'*40,source_manifest='candidate-source.json',build='synthetic'),
                            reference=dict(path='reference',revision=oc.REFERENCE_REVISION,source_manifest='reference-source.json',build='synthetic'))
                q.write(root/'config.json',config)
                frozen=moments.freeze(root/'config.json',root/'plan.json')
                self.assertEqual(frozen['call_cap'],624)
                self.assertEqual(frozen['expected_moments']['weak-robust']['constructed_moment_rows'],120)
                for name,digest in panel['files'].items():
                    self.assertEqual(frozen['files']['inputs/'+name],digest)
                broken=copy.deepcopy(panel);broken['files']['weak-robust.case']='0'*64
                q.write(root/'inputs/panel.json',broken)
                with self.assertRaisesRegex(ValueError,'frozen supplementary input changed'):
                    moments.freeze(root/'config.json',root/'bad-hash.json')
                source=root/'inputs/weak-robust.case';metadata=source.with_suffix('.json')
                original_bytes=source.read_bytes();original_meta=q.read(metadata)
                for key,value in [('vce','iid'),('memory','low')]:
                    source.write_bytes(original_bytes)
                    with source.open('rb') as stream:
                        shape,args=q.case_header(stream);body=stream.read()
                    args=[(key+'='+value).encode() if arg.startswith((key+'=').encode()) else arg for arg in args]
                    framed=b'FEREG_REPLAY_V1\n'+(' '.join(map(str,shape))+'\n').encode()
                    framed+=b''.join(str(len(arg)).encode()+b'\n'+arg+b'\n' for arg in args)+body
                    source.write_bytes(framed)
                    meta={**original_meta,'case_sha256':q.digest(source)};q.write(metadata,meta)
                    amended=copy.deepcopy(panel)
                    amended['files'][source.name]=q.digest(source)
                    amended['files'][metadata.name]=q.digest(metadata)
                    q.write(root/'inputs/panel.json',amended)
                    # Hashes are internally consistent: the fixed case identity,
                    # rather than a stale hash, must reject this altered model.
                    with self.subTest(field=key),self.assertRaisesRegex(ValueError,'coverage request contract changed'):
                        moments.freeze(root/'config.json',root/('bad-'+key+'.json'))


class CoverageRunner(unittest.TestCase):
    def setUp(self):
        legacy.Runner.setUp(self)
        patch.object(oc.routing_cases,'scratch',side_effect=lambda p:Path(p).resolve()).start()
        self.plan=plan_fixture()
        args=[b'estimate-ols',b'threads=1',b'solver=auto',b'preconditioner=auto']
        header=legacy.case_bytes(args).split(b'body\n',1)[0]
        numeric=b''.join(b'0\n'+b''.join(struct.pack('>d',float(v)).hex().encode()+b'\n' for v in column) for column in self.data)
        for item in self.plan['cases']:
            path=self.root/item['input'];path.write_bytes(header+numeric+b'1\nM\n1 1\n0000000000000000\n')
            self.files[item['input']]=q.digest(path)
        reference=self.root/'reference-plugin';reference.write_bytes(b'accepted')
        self.files[reference.name]=q.digest(reference)
        self.plan.update(artifacts={'candidate':{'path':'plugin'},'reference':{'path':reference.name}},
                         harness='harness',files=self.files,inputs='weak-robust.case',
                         execution_sources={n:q.digest(p) for n,p in oc.sources().items()})
        self.real_write(self.manifest,self.plan)

    def execute(self,command,log,timeout):
        result=legacy.Runner.execute(self,command,log,timeout)
        target=Path(command[-1]);arm=Path(command[-2]).stem
        if 'FEREG_PHASE_TIMING=1' in command and arm!='reference':
            setting=next(s for s in moments.settings() if s['cell']==target.parent.name)
            with log.open('a') as stream:
                for event in events(self.plan,setting,arm):
                    stream.write('FEREG_OWNERSHIP|'+ '|'.join(f'{k}={v}' for k,v in event.items())+'\n')
        if 'FEREG_SOLVER_TRACE=1' in command:
            used = 100 + transport_bytes(self.plan, arm)
            with log.open('a') as stream:
                stream.write(f'FEREG_SOLVER_TRACE|phase=routing_action|preset=GATES|final=1|iterations=0|native_limit=100|memory_bytes={used}|memory_limit=1000|certified=1\n')
        return result

    def test_missing_and_failed_preflight_prevent_any_executor_call(self):
        pre = self.root/'moments-preflight'/'results.json'
        for index,path in enumerate([None,pre,pre]):
            if index == 2:
                pre.parent.mkdir()
                self.real_write(path,dict(kind=moments.KIND,status='failed'))
            output=self.root/f'blocked-{index}'
            with self.assertRaises((ValueError,FileNotFoundError)):
                oc.run(self.manifest,output,preflight_result=path,execute=self.execute,monitor_factory=legacy.Monitor)
            self.assertEqual(self.calls,[])
            self.assertEqual(q.read(output/'results.json')['native_calls_started'],0)
            self.assertEqual(q.read(output/'results.json')['status'],'failed')

    def test_preflight_array_and_request_tamper_fail_before_timing(self):
        pre=self.root/'moments-preflight'
        prepared=oc.run(self.manifest,pre,diagnostic=True,execute=self.execute,monitor_factory=legacy.Monitor)
        reference=prepared['cells']['weak-robust-t1']['references']['B']
        bank=pre/Path(reference['path']).with_suffix('.npz')
        original=bank.read_bytes()
        with np.load(bank,allow_pickle=False) as stored:
            arrays={key:stored[key].copy() for key in stored.files}
        arrays['1'][0] ^= np.uint64(1)
        np.savez(bank,**arrays)
        with self.assertRaisesRegex(ValueError,'retained arrays changed'):
            oc.run(self.manifest,self.root/'tampered-array',preflight_result=pre/'results.json',
                   execute=self.execute,monitor_factory=legacy.Monitor)
        self.assertEqual(len(self.calls),108)
        # Even rebinding the mutated archive hash cannot bless changed output bits.
        altered=copy.deepcopy(prepared)
        altered['cells']['weak-robust-t1']['references']['B']['outputs']=q.digest(bank)
        with self.assertRaisesRegex(ValueError,'reconstructed scientific identity'):
            moments.verify_arrays(self.plan,self.manifest,pre,altered)
        bank.write_bytes(original)
        request=pre/'weak-robust-t1'/'moments.case'
        request.write_bytes(request.read_bytes().replace(b'inject=ownership_moments',b'inject=ownership_scores'))
        with self.assertRaises((ValueError,UnicodeError)):
            moments.validate_preflight(self.plan,self.manifest,pre/'results.json')
        self.assertEqual(len(self.calls),108)

    def test_complete_fake_preflight_then_timing_and_bound_report(self):
        pre=self.root/'moments-preflight'
        prepared=oc.run(self.manifest,pre,diagnostic=True,execute=self.execute,monitor_factory=legacy.Monitor)
        self.assertEqual(len(self.calls),108)
        self.assertEqual(prepared['execution_evidence']['status'],'passed')
        self.assertEqual(len(list(pre.rglob('*.npz'))),60)
        actual=oc.run(self.manifest,self.output,preflight_result=pre/'results.json',execute=self.execute,monitor_factory=legacy.Monitor)
        self.assertEqual(len(self.calls),732)
        self.assertEqual(actual['native_calls_started'],624)
        self.assertEqual(len(actual['cells']),12)
        self.assertEqual(actual['preflight_result_sha256'],q.digest(pre/'results.json'))
        self.assertTrue(all(c['timing_field']=='NATIVE_SECONDS' and c['seconds']==.01 for c in actual['calls']))
        self.assertEqual(len(list(self.output.rglob('*.npz'))),48)
        report=oc.report(self.manifest,self.output,self.root/'report.json')
        self.assertEqual(len(report['comparisons']),36)
        self.assertEqual(report['equal_weight_mean_difference_s'],dict(moments=0.,scores=0.,both=0.))
        self.assertFalse(report['activation_qualified'])
        self.assertEqual(report['configuration_O'],'both')
        self.assertTrue(report['discovery_nonregression'])
        self.assertGreaterEqual(self.oracle.call_count,732)
        with self.assertRaisesRegex(ValueError,'no retries'):
            oc.run(self.manifest,self.output,preflight_result=pre/'results.json',execute=self.execute,monitor_factory=legacy.Monitor)
        self.assertEqual(len(self.calls),732)


if __name__=='__main__':
    unittest.main()
