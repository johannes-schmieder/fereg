"""Synthetic prospective qualification contracts; no native process or real input generation."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify as q
import qualify_ownership as o
import routing
from test_cmgschur import case_bytes, result, bits
import test_qualify as historical


def manifest(platform='Darwin', request='ownership_both'):
    cases = [dict(id=f, family=f, input=f+'.case', oracle={'kind':'inactive' if f in
                  ('inactive-columns','ownership-inactive-zero') else 'routing','path':f+'.oracle.json'},
                  memory_limit_bytes=None) for f in sorted(q.FAMILIES)+list(o.ADDED)]
    roles = {'original':(o.ORIGINAL,'auto',[]), 'incumbent':(o.INCUMBENT,'auto',['many','two']),
             'candidate':('a'*40,request,['many','two'])}
    linux = platform == 'Linux'
    return dict(qualification=o.MODE, ownership_binding=dict(baseline_record='baseline.json',
                **{role+'_'+kind: role+'-'+kind+'.json' for role in roles for kind in ('source','build')}),
                coverage='ownership', scope='exact' if request=='ownership_both' else 'two',
                platform=platform, threads=[1,4], blocks=2 if linux else 24,
                shards=3 if linux else 1, replicas=12 if linux else 1,
                seed=2026106306, timeout_s=120,budget_s=3600 if linux else 14400,
                cases=cases, artifacts={r:dict(path=r+'.plugin',revision=rev,request=req,families=fam,build='synthetic 64-bit')
                                       for r,(rev,req,fam) in roles.items()},
                **({'min_distinct_hosts_per_cell':12} if linux else {}))


class Contracts(unittest.TestCase):
    def test_explicit_58_cells_fixed_budgets_and_no_extra_mac_retry_calls(self):
        for platform in ('Linux','Darwin'):
            value=manifest(platform);o.validate(value)
            calls=len(value['cases'])*len(value['threads'])*3*(1+2*value['blocks'])*value['replicas']
            self.assertEqual(calls,10440 if platform=='Linux' else 8526)
        value=manifest();value['files']={a['path']:str(i)*64 for i,a in enumerate(value['artifacts'].values(),1)}
        policy=q.fresh_mac_policy(value)
        self.assertEqual((policy['baseline_calls'],policy['new_call_cap'],policy['max_retry_cells'],policy['max_cell_retries']),
                         (8526,8526,0,0))
        original=copy.deepcopy(value);original.pop('qualification');original.pop('ownership_binding');original['cases']=original['cases'][:21]
        old=q.fresh_mac_policy(original)
        self.assertEqual((old['baseline_calls'],old['new_call_cap'],old['max_retry_cells']),(6174,6762,4))
        for edit in (lambda v:v.update(threads=[1,4,8]),lambda v:v.update(blocks=12),
                     lambda v:v.update(budget_s=14401),lambda v:v.update(scope='two'),
                     lambda v:v['cases'].pop(),lambda v:v.update(continuation={}),
                     lambda v:v['artifacts']['incumbent'].update(request='incremental-two'),
                     lambda v:v['artifacts']['candidate'].update(request='routing_ownership_both')):
            bad=manifest();edit(bad)
            with self.assertRaises(ValueError):o.validate(bad)
        bad=manifest('Linux');bad['min_distinct_hosts_per_cell']=11
        with self.assertRaises(ValueError):o.validate(bad)
        for request in o.CELL_REQUESTS:
            o.validate(manifest(request=request))

    def test_tokens_are_not_routing_presets_and_preserve_numeric_frame(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);source=root/'source.case';source.write_bytes(case_bytes())
            for workers in (1,4):
                old=root/f'old-{workers}.case';new=root/f'new-{workers}.case'
                routing.prepare(source,old,'baseline',workers);o.prepare(source,new,'auto',workers)
                self.assertEqual(old.read_bytes(),new.read_bytes())
                with old.open('rb') as stream:base_shape,base=q.case_header(stream);body=stream.read()
                for token in o.REQUESTS:
                    target=root/f'{token}-{workers}.case';argv=o.prepare(source,target,token,workers)
                    with target.open('rb') as stream:shape,args=q.case_header(stream);tail=stream.read()
                    self.assertEqual(args,base+[('inject='+token).encode()]);self.assertEqual(argv,args)
                    self.assertEqual(shape[2],base_shape[2]+1);self.assertEqual(tail,body)
                    self.assertEqual(o.argument_parent(args)-o.argument_parent(base),24+len('inject='+token))
                    with self.assertRaises(ValueError):routing.policy(token)
                    with self.assertRaises(ValueError):o.prepare(target,root/'bad.case',token,workers)
                    (root/'bad.case').unlink()

    def test_transport_uses_actual_argv_and_checks_raw_caps_first(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'source.case';p.write_bytes(case_bytes())
            with p.open('rb') as stream:_,source=q.case_header(stream)
        for token in o.REQUESTS:
            args=o.arguments(source,token,4);parent=o.argument_parent(args)
            action=dict(final='0',preset='routing_two_gates_v1',iterations_used=16,iterations_limit=100,
                        memory_bytes_used=parent+1234,memory_bytes_limit=parent+2000)
            value=o.transport_evidence(args,source,token,4,[action]);delta=24+len('inject='+token)
            self.assertEqual(value['argument_parent_delta_bytes'],delta)
            self.assertEqual(value['normalized_action_resources'][0]['memory_bytes_used'],parent+1234-delta)
            self.assertEqual(action['memory_bytes_used'],parent+1234)
            for changed in ({**action,'memory_bytes_used':parent-1},
                            {**action,'memory_bytes_limit':parent+1233},
                            {**action,'iterations_used':101}):
                with self.assertRaises(ValueError):o.transport_evidence(args,source,token,4,[changed])
            with self.assertRaises(ValueError):o.transport_evidence(args+[b'inject=bad'],source,token,4,[action])
            with self.assertRaises(ValueError):o.transport_evidence(args,source,token,1,[action])
            with self.assertRaises(ValueError):o.arguments(source+[b'threads=4'],token,4)

    def test_c16_and_candidate_use_gates_but_original_does_not(self):
        value=manifest()
        meta=dict(categorical_intercepts=True,absorbed_terms=2)
        self.assertIsNone(q.resource_policy(value,'original',meta))
        for role in ('candidate','incumbent'):
            self.assertEqual(q.resource_policy(value,role,meta),routing.GATES)
            self.assertEqual(q.resource_policy(value,role,{**meta,'absorbed_terms':3}),'incremental-many')
            self.assertIsNone(q.resource_policy(value,role,{**meta,'absorbed_terms':1}))
        # Outside ownership mode a public incumbent attests its declared public
        # two-FE policy; without the field it is gates v1.
        historical=copy.deepcopy(value);historical.pop('qualification')
        self.assertEqual(q.resource_policy(historical,'incumbent',meta),routing.GATES)
        historical['artifacts']['incumbent']['public_policy']=routing.GATES_V2
        self.assertEqual(q.resource_policy(historical,'incumbent',meta),routing.GATES_V2)

    def test_changed_cells_scope_keeps_routes_and_other_discrete_fields_exact(self):
        for request in o.CELL_REQUESTS:
            value=manifest(request=request)
            meta=dict(categorical_intercepts=True,absorbed_terms=2)
            before=result();after=copy.deepcopy(before)
            after['matrices']['RSOLVER']['bits'][2]=bits(7.)
            def snapshot(raw, fingerprint, hierarchy=None):
                return (raw,None,dict(fingerprint=fingerprint,gate_events=[],
                        action_resources=[dict(final='0',preset='routing_two_gates_v1',iterations_used=15,
                                               iterations_limit=100,memory_bytes_used=1000,memory_bytes_limit=2000)],
                        route_trace=[] if hierarchy is None else [{'installed':'1'}],
                        route_hierarchy=[] if hierarchy is None else [hierarchy]))
            snapshots=dict(original=snapshot(before,'baseline'),incumbent=snapshot(before,'baseline'),
                           candidate=snapshot(after,'changed'))
            # Mathematics is checked independently by the maintained native/oracle
            # harness. Here actual compatibility code tests the declared leaf scope.
            with patch.object(q.cmgschur,'scientific_comparison'):
                o.compare_artifacts(value,meta,snapshots)
                if request=='cells_deferred_both':
                    for changed_actions in ([], [{**snapshots['candidate'][2]['action_resources'][0],'final':'1'}],
                                            [{**snapshots['candidate'][2]['action_resources'][0],'iterations_limit':101}]):
                        bad=copy.deepcopy(snapshots);bad['candidate'][2]['action_resources']=changed_actions
                        with self.assertRaises(ValueError):o.compare_artifacts(value,meta,bad)
                    changed_work=copy.deepcopy(snapshots)
                    changed_work['candidate'][2]['action_resources'][0].update(iterations_used=16,memory_bytes_used=1500)
                    o.compare_artifacts(value,meta,changed_work)
                changed=copy.deepcopy(after);changed['matrices']['RSOLVER']['bits'][1]=bits(3.)
                bad={**snapshots,'candidate':snapshot(changed,'changed')}
                with self.assertRaises(ValueError):o.compare_artifacts(value,meta,bad)
                changed=copy.deepcopy(after);changed['matrices']['RFE']['bits'][0]=bits(2.)
                with self.assertRaises(ValueError):o.compare_artifacts(value,meta,{**snapshots,'candidate':snapshot(changed,'changed')})
                with self.assertRaises(ValueError):o.compare_artifacts(value,{**meta,'absorbed_terms':1},snapshots)
                with self.assertRaises(ValueError):o.compare_artifacts(manifest(),meta,snapshots)
                built={**snapshots,'incumbent':snapshot(before,'baseline',{'levels':2}),
                       'candidate':snapshot(after,'changed',{'levels':3})}
                with self.assertRaises(ValueError):o.compare_artifacts(value,meta,built)

    def test_literal_token_attests_gates_and_raw_cap_not_incremental_preset(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'source.case';path.write_bytes(case_bytes())
            with path.open('rb') as stream:_,source=q.case_header(stream)
        value=manifest();argv=o.arguments(source,'ownership_both',4);parent=o.argument_parent(argv)
        log=('FEREG_SOLVER_TRACE|phase=routing_action|final=0|preset=routing_two_gates_v1|'
             f'iterations=15|native_limit=100|memory_bytes={parent+100}|memory_limit={parent+1000}|certified=1\n')
        meta=dict(categorical_intercepts=True,absorbed_terms=2)
        detail=o.resource_detail(value,'candidate',meta,log,argv,source,4)
        self.assertEqual(detail['gate_events'],[])
        self.assertEqual(detail['ownership_transport']['argument_parent_delta_bytes'],45)
        for wrong in (log.replace('routing_two_gates_v1','routing_incremental_two'),
                      log.replace(f'memory_limit={parent+1000}',f'memory_limit={parent+99}'),
                      log.replace('certified=1','certified=0')):
            with self.assertRaises(ValueError):o.resource_detail(value,'candidate',meta,wrong,argv,source,4)

    def test_deferred_raw_trace_pairs_install_with_attempt_in_same_action(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'source.case';path.write_bytes(case_bytes())
            with path.open('rb') as stream:_,source=q.case_header(stream)
        value=manifest(request='cells_deferred_both')
        argv=o.arguments(source,'cells_deferred_both',4);parent=o.argument_parent(argv)
        meta=dict(categorical_intercepts=True,absorbed_terms=2)
        action=('FEREG_SOLVER_TRACE|phase=routing_action|final=0|preset=routing_two_gates_v1|'
                f'iterations=15|native_limit=100|memory_bytes={parent+100}|memory_limit={parent+1000}|certified=1')
        detail=o.resource_detail(value,'candidate',meta,action,argv,source,4)
        self.assertEqual(detail['ownership_transport']['argument_parent_delta_bytes'],50)
        self.assertEqual(detail['deferred_commitment'],dict(installations=0,attempts=0))
        for terms in (0,1):
            unaffected=o.resource_detail(value,'candidate',{**meta,'absorbed_terms':terms},'',argv,source,4)
            self.assertNotIn('deferred_commitment',unaffected)
        installed='FEREG_SOLVER_TRACE|phase=auto_schur_route|installed=1|refusal=None'
        attempt='FEREG_CELLS|event=attempt|attempted=1|admitted=1'
        # Isolate the commitment validator's wiring from the separate full
        # hierarchy/GATES schema tests; actual argv and raw caps still execute.
        with patch.object(o.science,'resource_diagnostics',return_value={
                'action_resources':detail['action_resources']}), patch('two_fe_policy_development.gate_events',return_value=[]):
            for event in (attempt,attempt.replace('admitted=1','admitted=0')):
                got=o.resource_detail(value,'candidate',meta,'\n'.join([installed,event,action]),argv,source,4)
                self.assertEqual(got['deferred_commitment'],dict(installations=1,attempts=1))
            for lines in ([attempt,installed,action], [installed,action,attempt],
                          [installed,action], [installed], [installed,attempt,attempt],
                          [installed.replace('installed=1','installed=0'),attempt,action]):
                with self.assertRaises(ValueError):
                    o.resource_detail(value,'candidate',meta,'\n'.join(lines),argv,source,4)

    def test_diagnostic_is_separate_frozen_proposal_not_scheduled_timing(self):
        value=manifest();proposal=o.diagnostic_proposal(value)
        self.assertEqual((proposal['native_calls'],proposal['budget_s'],proposal['timeout_s'],proposal['retries']),
                         (80,2400,120,0))
        self.assertFalse(proposal['performance_selection'])
        for f in o.ADDED:
            for w in (1,4):
                rows=[r for r in proposal['schedule'] if r['cell']==f'{f}-t{w}']
                self.assertEqual([r['arm'] for r in rows],['incumbent','uninjected_candidate','uninjected_candidate','candidate','candidate'])
        self.assertNotIn('diagnostic',value)

    def test_inactive_family_requires_new_mode_and_real_existing_witness(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);source=root/'source.case';source.write_bytes(case_bytes());where=root/'plan.json'
            item=dict(input='source.case',metadata=None,family='ownership-inactive-zero',oracle={'kind':'inactive'})
            inspected=dict(rows=2,columns=6,body_sha256='1'*64)
            with patch.object(q.routing_cases,'inspect_case',return_value=inspected), patch.object(q.routing_cases,'inactive_case_oracle',return_value={'witness':True}) as check:
                self.assertEqual(q.metadata({'qualification':o.MODE},item,where)['oracle'],{'witness':True})
                check.assert_called_once_with(source.resolve())
                with self.assertRaises(ValueError):q.metadata({},item,where)
            with patch.object(q.routing_cases,'inspect_case',return_value=inspected):
                with self.assertRaises(ValueError):q.metadata({'qualification':o.MODE},item,where)

    def test_bound_original_inputs_references_and_transport_sources_cannot_change(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);value=manifest()
            retained=Path(__file__).resolve().parents[1]/'results/two-fe-policy-gates-qualification-freeze-20261022.json'
            record=q.read(retained);old=record['plans']['Darwin']['content']
            q.write(root/'baseline.json',record)
            value['cases']=copy.deepcopy(old['cases'])+value['cases'][21:]
            value['files']=dict(old['files'])
            # A newly source-bound f35 build may differ from historical bytes.
            value['files']['original.plugin']='a'*64
            value['files'].update({'incumbent.plugin':'b'*64,'candidate.plugin':'c'*64})
            for role in ('original','candidate','incumbent'):
                q.write(root/f'{role}-source.json',dict(fereg=value['artifacts'][role]['revision'],cmg='d'*40,
                            archive_sha256='e'*64,files=({'historical/source.rs':'f'*64} if role=='original' else o.TRANSPORT_SOURCES)))
                q.write(root/f'{role}-build.json',dict(fereg=value['artifacts'][role]['revision'],cmg='d'*40,
                            plugin_sha256=value['files'][role+'.plugin'], source_hashes_verified_before_after=True,
                            native_abi_load_panic='passed',build=value['artifacts'][role]['build']))
            for key,path in value['ownership_binding'].items():value['files'][path]=q.digest(root/path)
            o.validate_bindings(value,root/'plan.json')
            for role in ('original','incumbent','candidate'):
                path=root/f'{role}-build.json';build=q.read(path)
                for field,wrong in (('fereg','0'*40),('cmg','0'*40),('plugin_sha256','0'*64),
                                    ('source_hashes_verified_before_after',False),('native_abi_load_panic','failed'),
                                    ('build','different compiler')):
                    q.write(path,{**build,field:wrong})
                    with self.assertRaises(ValueError):o.validate_bindings(value,root/'plan.json')
                q.write(path,build)
            for item,key in ((value['cases'][0],'input'),(next(c for c in value['cases'] if c['oracle']['kind']=='routing')['oracle'],'arrays')):
                bad=copy.deepcopy(value);bad['files'][item[key]]='0'*64
                with self.assertRaises(ValueError):o.validate_bindings(bad,root/'plan.json')
            source=q.read(root/'candidate-source.json');source['files']=dict(source['files']);source['files']['crates/fereg-plugin/src/ffi.rs']='0'*64
            q.write(root/'candidate-source.json',source)
            with self.assertRaises(ValueError):o.validate_bindings(value,root/'plan.json')

    def test_added_frozen_shape_metadata_unit_layout_and_oracle_allowance(self):
        for family in o.ADDED:
            spec=o.added_cases.specification(family); n,k=spec['rows'],spec['regressors']
            parsed=dict(rows=n,columns=k+10,x=np.broadcast_to(0.,(n,k)),y=np.broadcast_to(0.,(n,)),
                        factors=[np.arange(n)%spec['workers'],np.arange(n)%spec['firms']],
                        weights=np.ones(n),include_intercept=spec['include_intercept'],vce='robust',
                        account_cluster_nesting=False,keep_singletons=False)
            meta=o.added_cases.specification_metadata(spec)
            item=dict(family=family,oracle={'kind':'inactive' if family=='ownership-inactive-zero' else 'routing'})
            ref=dict(rows=n,retained_rows=n,regressors=k,include_intercept=spec['include_intercept'],
                     certificate=dict(levels=[spec['workers'],spec['firms']],
                                      memory_budget_bytes=spec['oracle_memory_budget_bytes']))
            truth=({'oracle':dict(regressors=k,fitted=np.zeros(n),residual=np.zeros(n))}
                   if item['oracle']['kind']=='inactive' else {'routing_oracle':ref})
            o.validate_added(item,meta,parsed,truth)
            for key,wrong in (('derived_seed',spec['derived_seed']+1),('random_streams',[0]),
                              ('row_transform','substituted'),('oracle_memory_budget_bytes',3<<30)):
                with self.assertRaises(ValueError):o.validate_added(item,{**meta,key:wrong},parsed,truth)
            # Unit-valued explicit weight columns are still the wrong layout.
            for bad in ({**parsed,'columns':k+11},{**parsed,'weights':np.full(n,2.)},
                        {**parsed,'x':np.broadcast_to(0.,(n,k+1))},
                        {**parsed,'factors':[parsed['factors'][0],np.zeros(n)]}):
                with self.assertRaises(ValueError):o.validate_added(item,meta,bad,truth)
            if item['oracle']['kind']=='routing':
                wrong={**ref,'certificate':{**ref['certificate'],'memory_budget_bytes':
                                          (1 if spec['oracle_memory_budget_bytes']==2<<30 else 2)<<30}}
                with self.assertRaises(ValueError):o.validate_added(item,meta,parsed,{'routing_oracle':wrong})
                wrong={**ref,'regressors':k+1}
                with self.assertRaises(ValueError):o.validate_added(item,meta,parsed,{'routing_oracle':wrong})
                wrong={**ref,'retained_rows':n-1}
                with self.assertRaises(ValueError):o.validate_added(item,meta,parsed,{'routing_oracle':wrong})
        self.assertIn('ownership_qualification_cases',q.execution_sources())
        self.assertIn('ownership_panel3',q.execution_sources())

    def test_exact_boundary_rows_not_labels(self):
        # Small column count; exact prescribed row lengths. No actual generated
        # qualification input or oracle is produced by this synthetic framing test.
        with tempfile.TemporaryDirectory() as tmp:
            paths=[Path(tmp)/f'{n}.case' for n in (262143,262144,262145)]
            args=[b'estimate-ols',b'threads=1'];row=b'3ff0000000000000\n'
            for path,n in zip(paths,(262143,262144,262145)):
                prefix=f'FEREG_REPLAY_V1\n{n} 1 2 1 0\n'.encode()+b''.join(str(len(a)).encode()+b'\n'+a+b'\n' for a in args)
                path.write_bytes(prefix+b'0\n'+row*n+b'common matrix tail')
            o.validate_boundary(paths)
            bad=paths[2].read_bytes().replace(b'3ff0000000000000',b'4000000000000000',1);paths[2].write_bytes(bad)
            with self.assertRaises(ValueError):o.validate_boundary(paths)


class RunnerIntegration(unittest.TestCase):
    def setUp(self):
        self.f=historical.Qualification('runTest');self.f.setUp();self.addCleanup(self.f.doCleanups)

    def test_real_runner_dispatch_retention_and_report_without_native(self):
        f=self.f;value=f.plan()
        for role in value['artifacts']:
            path=f.root/(role+'.plugin');path.write_bytes(role.encode())
            value['artifacts'][role]['path']=path.name;value['files'][path.name]=q.digest(path)
        value.update(qualification=o.MODE,ownership_binding={})
        value['artifacts']['candidate']['request']='ownership_both'
        q.write(f.manifest,value)
        # Fixed 58-cell budgets/bindings are independently tested above. This
        # two-worker synthetic fixture exercises actual runner/report hooks.
        with patch.object(o,'validate'),patch.object(o,'validate_bindings'):
            record=q.run(f.manifest,f.output,execute=f.execute)
            self.assertEqual(record['status'],'complete',record['failures'])
            self.assertEqual(len(f.calls),30)
            for cell in record['cases']:
                self.assertEqual(len(cell['warmups']),3)
                for call in cell['warmups']:
                    self.assertIn('ownership_transport',call)
                    self.assertIn('raw_result',call)
                for block in cell['blocks']:
                    self.assertTrue(all(c['timing_field']=='NATIVE_SECONDS' for c in block['calls']))
            report=q.report(f.manifest,f.output)
            self.assertEqual(report['status'],'smoke_complete',report['errors'])
            self.assertFalse(report['activation_qualified'])
            cell=record['cases'][0];call=cell['warmups'][2]
            log=f.output/'run-0-0'/call['log'];log.write_text(log.read_text()+'tampered\n')
            report=q.report(f.manifest,f.output)
            self.assertEqual(report['status'],'needs_discussion')
            self.assertTrue(any('identity mismatch' in error for error in report['errors']))


if __name__=='__main__':unittest.main()
