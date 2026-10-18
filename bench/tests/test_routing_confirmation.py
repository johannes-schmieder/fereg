"""Synthetic proof boundaries only: never open an actual case or run old admission."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import routing
import routing_blocks as blocks
import routing_block_deployment as deployment
import routing_confirmation as confirmation


def h(value): return hashlib.sha256(value.encode()).hexdigest()


class FreshConfirmation(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve(); self.serial = 0
        self.source = self.write({'unchanged': 'source'})
        def cells(stage):
            return [{'id': f'{stage}-{i}-t{worker}', 'case_file': f'{stage}-{i}.case',
                     'case_sha256': h(f'{stage}-case-{i}'), 'body_sha256': h(f'{stage}-body-{i}'),
                     'threads': worker, 'family': f'family-{i}', 'coverage': ['many-fe'],
                     'comparators': ['lsmr'], 'categorical_intercepts': True, 'absorbed_terms': 3}
                    for i in range(21) for worker in (1,4)]
        self.original = {'panels': {stage: cells(stage) for stage in ('A','B','C')},
            'incremental': {'family':'many'}, 'experiment_kind':'incremental', 'prior_manifests':[],
            'arms': {'baseline':{'sha256':h('original')},'incumbent':{'sha256':h('incumbent')},
                     'incremental-many':{'sha256':h('hidden'),'policy':{'family':'many'}},
                     'lsmr':{'sha256':h('hidden')}},
            'protections':['baseline','incumbent'], 'harness_sha256':h('harness'),
            'failure_policy': {'per_call_timeout_s':120,'aggregate_budget_s':14400}}
        self.design = {'attempts':24,'warmups':1,'seed':133,'timeout_s':120,'budget_s':28800,
                       'session_id':'old-C','analysis_unit':'block','alpha':{'numerator':1,'denominator':40}}
        b_context = self.write(routing.seal({'design':{**self.design,'seed':132,'session_id':'old-B'}}))
        self.inputs = {key:self.source for key in deployment.INPUTS}
        self.inputs['declaration'] = self.write(routing.seal({'b_context':b_context}))
        manifest = deployment.holdout_manifest(self.original,'incremental-many',{'sha256':h('public')})
        self.old = {'schema':1,'kind':deployment.KIND,'stage':'C','status':'activated',
            'experiment_kind':'incremental','inputs':self.inputs,'selected':'incremental-many',
            'manifest':manifest,'design':self.design,'cells':self.original['panels']['C'],
            'candidates':[deployment.PUBLIC],'protections':['baseline','incumbent','selected-hidden'],
            'allowances':{'protected':{'relative':.05,'absolute_seconds':.001}},'stopping':'historical'}
        self.failure = self.write({'context':self.write(routing.seal(self.old))})
        self.basis = {'kind':'actual-failure','status':'not_confirmed','native_calls':0,'timed_calls':0}
        self.enterContext(patch.object(confirmation.failure,'failure_basis',return_value=(self.original,self.basis)))
        self.actual_failure_dependencies = confirmation.failure_dependencies
        self.enterContext(patch.object(confirmation,'failure_dependencies',return_value=[self.failure]))
        self.derive_mock = self.enterContext(patch.object(deployment,'derive',side_effect=lambda inputs: copy.deepcopy(self.old)))
        self.partial_identity = h('actual-canonical-partial-return')
        self.actual_partial_tuple_identity = confirmation.partial_tuple_identity
        self.enterContext(patch.object(confirmation,'partial_tuple_identity',return_value=self.partial_identity))
        sources = {key:self.source for key in confirmation.SOURCE_KEYS}
        python_command = self.root/'venv'/'bin'/'python'
        python_command.parent.mkdir(parents=True)
        python_command.symlink_to(sys.executable)
        python_environment = python_command.parent.parent/'pyvenv.cfg'
        python_environment.write_text('home = synthetic-test-environment\n')
        sources.update(confirmation=blocks.reference(confirmation.__file__),
                       validation=blocks.reference(Path(confirmation.__file__).with_name('routing_validation.py')),
                       failure_validator=blocks.reference(confirmation.failure.__file__),
                       python_environment=blocks.reference(python_environment),
                       python=blocks.reference(sys.executable))
        many = {'schema':1,'family':'many','panels':self.original['panels'],
                'seeds':{'A':111,'B':112,'C':113},'generator_sha256':{self.source['path']:self.source['sha256']}}
        two = {'schema':1,'family':'two','panels':{stage:cells('two'+stage) for stage in ('A','B','C')},
               'seeds':{'A':121,'B':122,'C':123}}
        panel_path = self.root/'fresh'/'c'/'panel.json'; imports = panel_path.parent.parent/'imports-c'
        panel_argv = [str(python_command),sources['panel_generator']['path'],'--panel','C','--seed','1801',
            '--output',str(panel_path.parent),'--generator',sources['case_generator']['path'],
            '--extended','--sampled','--compact-it','--low-saved',str(imports/'low-saved.case')]
        for family in ('no-fe','one-fe','slopes'): panel_argv += ['--source',family+'='+str(imports/(family+'.case'))]
        self.config = {'original_inputs':self.inputs,'confirmation_failure':self.failure,
            'authorization':{'spec':self.source,'workflow':self.source},'repair_review':self.source,
            'history':{'many_inputs':self.write(many),'two_inputs':self.write(two)},
            'design':{**self.design,'seed':1802,'session_id':'fresh-C'},'limits':dict(confirmation.LIMITS),
            'sources':sources,'input_seed':1801,'declared_unix':100.,'fresh_panel_path':str(panel_path),
            'python_command':str(python_command),
            'generation_commands':[{'role':'imports','argv':[sources['imports_generator']['path'],str(imports),'1801']},
                                   {'role':'panel','argv':panel_argv}]}
        path = blocks.checked(self.source); stat = path.stat()
        self.report = {'kind':confirmation.KIND+'_validation','status':'passed',
            'original_context_sha256':routing.identity(self.old),'original_inputs':self.inputs,
            'sources':sources,'checks':{key:True for key in confirmation.CHECKS},'old_B_wrappers':42,
            'canonical_partial_tuple_sha256':self.partial_identity,
            'repaired_partial_tuple_sha256':self.partial_identity,'completed_unix':90.,
            'metadata':{'poisoned':None,'full_byte_checks':2,'closure':[{**self.source,
                'stamp':[stat.st_dev,stat.st_ino,stat.st_mode,stat.st_size,stat.st_mtime_ns,stat.st_ctime_ns]}],
                'path_aliases':{str(path):str(path)}},'evidence':[self.source]}
        self.review = {'kind':confirmation.KIND+'_review','status':'passed',
            'completed_unix':95.,
            'proposal_sha256':confirmation.proposal_identity(self.config),'failure_basis_sha256':routing.identity(self.basis),
            'sources':sources,'validation_report':self.write(self.report),'evidence':[self.source]}
        self.config['repair_review'] = self.write(self.review)
        self.declaration = self.write(routing.seal(confirmation.declare(self.config)))
        fresh = [{**cell,'case_sha256':h('fresh-'+cell['case_sha256']),
                          'body_sha256':h('fresh-'+cell['body_sha256'])} for cell in self.old['cells']]
        self.panel = {'schema':1,'panel':'C','seed':1801,'cells':fresh,'compact_it_comparator':True,
                      'stopping_rule':'one fixed sparse panel; no Cartesian expansion'}
        panel_path.parent.mkdir(parents=True); panel_path.write_text(json.dumps(self.panel)); self.panel_ref = blocks.reference(panel_path)
        self.receipt = {'kind':confirmation.KIND+'_generation','status':'complete','declaration':self.declaration,
            'repair_review':self.config['repair_review'],'panel':self.panel_ref,'commands':self.config['generation_commands'],
            'sources':sources,'seed':1801,'started_unix':110.,'completed_unix':120.,'native_calls':0,'timed_calls':0,
            'evidence':[self.source]}
        self.receipt_ref = self.write(self.receipt)
        self.context = confirmation.derive(self.declaration,self.panel_ref,self.receipt_ref)
        self.context_ref = self.write(routing.seal(self.context))

    def write(self,value):
        self.serial += 1;path=self.root/f'{self.serial}.json';path.write_text(json.dumps(value));return blocks.reference(path)

    def test_derivation_preserves_every_old_selected_artifact_and_activation_input(self):
        self.assertEqual(self.context['inputs'],self.inputs)
        self.assertEqual(self.context['manifest']['arms'],self.old['manifest']['arms'])
        self.assertEqual(self.context['protections'],self.old['protections'])
        self.assertEqual(self.context['selected'],'incremental-many')
        self.assertEqual(self.context['manifest']['panels']['C'],self.panel['cells'])
        self.assertEqual(self.old['cells'],self.original['panels']['C'])
        self.assertNotEqual(self.context['cells'],self.old['cells'])
        self.assertIn('engineering blocker',self.context['stopping'])
        self.assertGreaterEqual(self.derive_mock.call_count,2)

    def test_partial_tuple_identity_comes_from_actual_old_partial_context_return(self):
        partial_inputs = {**self.inputs, 'activation': self.write({'partial': True})}
        context = self.write(routing.seal({'inputs': partial_inputs}))
        equivalent = self.write({'collections': [{'context': context}]})
        complete = {**self.inputs, 'activation': self.write({'reports': {'equivalence': equivalent}})}
        actual_return = ({'declared': True}, {'original': True}, 'incremental-many',
                         {'partial': True}, {'trace': True}, {'public': True})
        with patch.object(deployment, 'activation_core', return_value=actual_return) as validate:
            self.assertEqual(self.actual_partial_tuple_identity(complete), routing.identity(actual_return))
            validate.assert_called_once_with(partial_inputs, equivalence=False)
        self.assertNotEqual(routing.identity(actual_return), routing.identity(complete))

    def test_failure_dependencies_freeze_all_raw_failure_and_admission_closures(self):
        refs = [self.write({'evidence': i}) for i in range(13)]
        start = dict(zip(('supervisor','owned_runner','witness_adapter','source_review'), refs[:4]))
        admitted = {'sources': {'old': refs[4]}, 'metadata': {'closure': [refs[5]]}}
        admission = {'supervisor_outcome': refs[6]}
        handoff = {'config_snapshot': refs[7], 'closure': [refs[8]]}
        review = dict(zip(('audit_source','closure_capture','lead_failure_validation',
                          'supervisor_log','witness_log'), refs[8:13]))
        packet = {'context': self.write(routing.seal({})), 'start': self.write(start),
                  'outcome': self.write({}), 'handoff': self.write(routing.seal(handoff)),
                  'admission_outcome': self.write(admitted), 'admission_review': self.write(admission),
                  'raw_review': self.write(review), 'partial_outputs': [self.source]}
        bound = self.write(packet)
        actual = self.actual_failure_dependencies(bound)
        for ref in [bound, self.source, *refs, *[packet[k] for k in confirmation.failure.REFERENCES]]:
            self.assertIn(ref, actual)

    def test_review_and_declaration_chronology_without_a_circular_proposal(self):
        changed = {**self.config, 'declared_unix': 101.}
        self.assertEqual(confirmation.proposal_identity(changed), confirmation.proposal_identity(self.config))
        for completed in (89., 101.):
            review = {**self.review, 'completed_unix': completed}
            with self.assertRaises(ValueError):
                confirmation.declare({**self.config, 'repair_review': self.write(review)})
        self.assertEqual(confirmation.declare(changed)['config']['declared_unix'], 101.)

    def test_all_limits_and_fixed_design_fields_fail_closed(self):
        for key in confirmation.LIMITS:
            bad=copy.deepcopy(self.config);bad['limits'][key]+=1
            with self.subTest(key=key),self.assertRaises(ValueError):confirmation.declare(bad)
        for key,value in [('attempts',26),('warmups',2),('budget_s',30000),('timeout_s',121),('alpha',{'numerator':1,'denominator':20})]:
            bad=copy.deepcopy(self.config);bad['design'][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):confirmation.declare(bad)

    def test_seeds_and_sessions_cannot_reuse_any_prior_panel_or_timing_design(self):
        for seed in [111,112,113,121,122,123,132,133,1801]:
            bad=copy.deepcopy(self.config);bad['design']['seed']=seed
            with self.subTest(seed=seed),self.assertRaises(ValueError):confirmation.declare(bad)
        for session in ['old-B','old-C']:
            bad=copy.deepcopy(self.config);bad['design']['session_id']=session
            with self.assertRaises(ValueError):confirmation.declare(bad)

    def test_generation_recipe_cannot_drop_imports_or_change_fixed_options(self):
        for mutation in ('missing','option','seed','generator'):
            bad=copy.deepcopy(self.config)
            if mutation=='missing':bad['generation_commands'].pop(0)
            elif mutation=='option':bad['generation_commands'][1]['argv'].remove('--sampled')
            elif mutation=='seed':bad['generation_commands'][0]['argv'][-1]='1800'
            else:bad['sources']['case_generator']=self.write({'other':'generator'})
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):confirmation.declare(bad)

    def test_virtual_environment_command_is_distinct_from_canonical_binary_binding(self):
        self.assertNotEqual(self.config['python_command'], self.config['sources']['python']['path'])
        self.assertEqual(self.config['generation_commands'][1]['argv'][0], self.config['python_command'])
        for mutation in ('command', 'environment', 'argv'):
            bad = copy.deepcopy(self.config)
            if mutation == 'command': bad['python_command'] = self.source['path']
            elif mutation == 'environment': bad['sources']['python_environment'] = self.source
            else: bad['generation_commands'][1]['argv'][0] = bad['sources']['python']['path']
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): confirmation.same_config(bad)

    def test_virtual_environment_command_alias_retarget_fails_closed(self):
        command = Path(self.config['python_command'])
        command.unlink(); command.symlink_to(self.source['path'])
        with self.assertRaisesRegex(ValueError, 'virtual-environment command'):
            confirmation.same_config(self.config)

    def test_fresh_cells_require_identical_shape_order_and_all_42_workers(self):
        histories,_=confirmation.history_metadata(self.config,self.original)
        for change in ('count','order','worker','family','comparator','extra','bool-schema'):
            bad=copy.deepcopy(self.panel)
            if change=='count':bad['cells'].pop()
            elif change=='order':bad['cells'].reverse()
            elif change=='worker':bad['cells'][0]['threads']=2
            elif change=='family':bad['cells'][0]['family']='other'
            elif change=='comparator':bad['cells'][0]['comparators']=[]
            elif change=='extra':bad['cells'][0]['unreviewed']=True
            else:bad['schema']=True
            with self.subTest(change=change),self.assertRaises(ValueError):confirmation.fresh_panel(bad,self.config,self.original,histories)

    def test_freshness_covers_all_many_two_and_recursive_earlier_panels(self):
        histories,_=confirmation.history_metadata(self.config,self.original)
        original=copy.deepcopy(self.original)
        old={'panels':{'A':[{'case_sha256':h('ancestral'),'body_sha256':h('ancestral-body')}]}}
        original['prior_manifests']=[routing.seal({'panels':{},'prior_manifests':[routing.seal(old)]})]
        for cells in [*self.original['panels'].values(),*histories['two_inputs']['panels'].values(),old['panels']['A']]:
            for key in ('case_sha256','body_sha256'):
                bad=copy.deepcopy(self.panel);bad['cells'][0][key]=cells[0][key]
                with self.subTest(hash=key),self.assertRaises(ValueError):confirmation.fresh_panel(bad,self.config,original,histories)

    def test_worker_variants_keep_identity_grouping(self):
        histories,_=confirmation.history_metadata(self.config,self.original)
        bad=copy.deepcopy(self.panel);bad['cells'][1]['body_sha256']=h('different-worker-body')
        with self.assertRaisesRegex(ValueError,'identity grouping'):confirmation.fresh_panel(bad,self.config,self.original,histories)

    def test_repair_review_is_bound_to_proposal_failure_and_new_sources(self):
        for key in ('proposal_sha256','failure_basis_sha256','sources','status','completed_unix'):
            review=copy.deepcopy(self.review);review[key]={} if key=='sources' else 'changed'
            bad={**self.config,'repair_review':self.write(review)}
            with self.subTest(key=key),self.assertRaises(ValueError):confirmation.declare(bad)

    def test_repair_requires_actual_old_admission_wrappers_tuple_and_drift_cleanup(self):
        for key in [*confirmation.CHECKS,'old_B_wrappers','canonical_partial_tuple_sha256','repaired_partial_tuple_sha256','original_context_sha256','full_byte_checks','closure']:
            report=copy.deepcopy(self.report)
            if key in confirmation.CHECKS:report['checks'][key]=False
            elif key=='old_B_wrappers':report[key]=41
            elif key in ('full_byte_checks','closure'):report['metadata'][key]=1 if key=='full_byte_checks' else []
            else:report[key]=h('changed')
            review={**self.review,'validation_report':self.write(report)}
            with self.subTest(key=key),self.assertRaises(ValueError):confirmation.declare({**self.config,'repair_review':self.write(review)})

    def test_actual_repair_read_closure_drift_fails_even_after_outer_rebinding(self):
        report=copy.deepcopy(self.report);report['metadata']['closure'][0]['stamp'][-1]+=1
        review={**self.review,'validation_report':self.write(report)}
        with self.assertRaisesRegex(ValueError,'closure changed'):confirmation.declare({**self.config,'repair_review':self.write(review)})

    def test_generation_requires_post_review_actual_identity_command_and_receipt(self):
        for key,value in [('started_unix',99.),('completed_unix',109.),('commands',[]),('native_calls',1),
                          ('timed_calls',1),('declaration',self.source),('repair_review',self.source),('evidence',[])]:
            with self.subTest(key=key),self.assertRaises(ValueError):
                confirmation.derive(self.declaration,self.panel_ref,self.write({**self.receipt,key:value}))

    def test_historical_dispatch_delegates_and_restores_even_on_exception(self):
        historical=self.write(routing.seal({'historical':'context'}))
        oldload=lambda bound:('historical',bound)
        oldbindings=lambda bound,context:[bound]
        with patch.object(blocks,'load_context',oldload),patch.object(blocks,'context_bindings',oldbindings):
            with self.assertRaises(RuntimeError):
                with confirmation.install():
                    self.assertEqual(blocks.load_context(historical),('historical',historical))
                    self.assertEqual(blocks.context_bindings(historical,{}),[historical])
                    raise RuntimeError('cleanup')
            self.assertIs(blocks.load_context,oldload);self.assertIs(blocks.context_bindings,oldbindings)

    def test_malformed_or_other_version_marker_never_delegates(self):
        with patch.object(blocks,'load_context',side_effect=AssertionError('unsafe historical delegate')):
            with confirmation.install():
                for marker in [None,{}, {'version':'v2'}, {**self.context[confirmation.MARKER],'version':'v2'}]:
                    with self.subTest(marker=marker),self.assertRaises(ValueError):
                        blocks.load_context(self.write(routing.seal({**self.context,confirmation.MARKER:marker})))

    def test_versioned_load_rederives_and_rejects_artifact_protection_or_input_drift(self):
        with confirmation.install():
            self.assertEqual(blocks.load_context(self.context_ref),(self.context,self.context['manifest']))
            for key,value in [('selected','incremental-two'),('protections',['baseline']),('inputs',{})]:
                with self.subTest(key=key),self.assertRaises(ValueError):
                    blocks.load_context(self.write(routing.seal({**self.context,key:value})))
            bad=copy.deepcopy(self.context);bad['manifest']['arms'][deployment.PUBLIC]['sha256']=h('retuned')
            with self.assertRaises(ValueError):blocks.load_context(self.write(routing.seal(bad)))

    def test_context_bindings_retain_every_new_source_review_and_generation_receipt(self):
        with patch.object(blocks,'context_bindings',return_value=[self.context_ref]):
            with confirmation.install(): refs=blocks.context_bindings(self.context_ref,self.context)
        for bound in [*self.config['sources'].values(),self.config['repair_review'],self.receipt_ref,self.panel_ref,self.declaration]:
            self.assertIn(bound,refs)

    def test_terminal_extra_history_keeps_original_manifest_truth(self):
        history=confirmation.terminal_history(self.context)
        terminal={'prior_manifest':routing.seal(self.original),'export_review':{'report':{'confirmation_history':history}}}
        with patch.object(deployment,'validate_terminal',return_value=terminal):
            self.assertEqual(confirmation.validate_terminal(terminal,portable=True),terminal)
            extra=confirmation.additional_history(terminal)
            self.assertEqual(set(extra['bodies']),{c['body_sha256'] for c in self.panel['cells']})
            self.assertEqual(routing.unseal(terminal['prior_manifest']),self.original)
            bad=copy.deepcopy(terminal);bad['export_review']['report'].pop('confirmation_history')
            with self.assertRaises(ValueError):confirmation.validate_terminal(bad,portable=True)


if __name__ == '__main__': unittest.main()
