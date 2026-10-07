"""Bounded screening cannot select favorable observations or skip evidence."""
import copy
import errno
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare import acceptance_envelopes, confirmation_scope, median_order_interval, validate_measurement_plan, scientific_accuracy, ReferenceCache
from run import command_solver, execute_cell, measure, save_campaign


class BoundedMeasurementTests(unittest.TestCase):
    def test_common_reference_retires_parent_construction_without_relabeling(self):
        args=SimpleNamespace(tier='pilot',threads_only=None,reverse_order=False,measurement_plan=Path('plan.json'),
            measurement_stage='screening',measurement_started=time.monotonic(),
            bounded_plan={'scientific_reference':{'role':'candidate'},'maximum_wall_seconds_per_block':1000})
        suite=dict(tiers={'pilot':{'threads':[4]}},comparators={'ols':['fereg']},
                   protocol={'reference_artifact':'candidate','repetitions':8})
        artifacts=[SimpleNamespace(artifact_role=r) for r in ['parent','candidate']]
        cell=dict(id='a',model='ols');records=[]
        def capture(artifact,*positional,**keywords):
            if keywords.get('reference'):
                self.assertEqual(artifact.artifact_role,'candidate')
            return dict(status='ok',artifact_role=artifact.artifact_role,reference=keywords.get('reference',False))
        with patch('run.measure',side_effect=capture):
            execute_cell(args,artifacts,suite,cell,Path('data'),{},records,lambda:None)
        self.assertEqual(len(records),19)
        self.assertEqual([r['artifact_role'] for r in records if r['reference']],['candidate'])
        self.assertEqual({r['artifact_role'] for r in records if not r['reference']},{'parent','candidate'})

    def test_common_science_detects_wrong_covariance_and_identified_outputs(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);reference=root/'reference';actual=root/'actual'
            for p in [reference,actual]:
                p.mkdir()
                (p/'science.hex').write_text('b.1 +1.0X+0\nV.1.1 +1.0X+0\n')
                (p/'scalars.hex').write_text('N +2.0X+0\nrank +1.0X+0\n')
                (p/'outputs.csv').write_text('obs_id,bench_mu,saved1,saved2\n1,1,.5,.5\n2,2,.6,.4\n')
            record={'directory':str(actual)};target={'directory':str(reference)}
            self.assertTrue(scientific_accuracy(record,target,ReferenceCache()))
            (actual/'outputs.csv').write_text('obs_id,bench_mu,saved1,saved2\n1,1,.75,.25\n2,2,.85,.15\n')
            self.assertTrue(scientific_accuracy(record,target,ReferenceCache()))
            for filename,payload in [('science.hex','b.1 +1.0X+0\nV.1.1 +2.0X+0\n'),
                ('scalars.hex','N +3.0X+0\nrank +1.0X+0\n'),
                ('outputs.csv','obs_id,bench_mu,saved1,saved2\n1,1,.75,.5\n2,2,.85,.4\n')]:
                original=(actual/filename).read_bytes();(actual/filename).write_text(payload)
                self.assertFalse(scientific_accuracy(record,target,ReferenceCache()),filename)
                (actual/filename).write_bytes(original)
            cache=ReferenceCache(maximum_bytes=4096)
            self.assertTrue(scientific_accuracy(record,target,cache))
            self.assertLessEqual(cache.retained_bytes,4096)

    def test_common_science_preserves_exact_omission_in_both_directions(self):
        with tempfile.TemporaryDirectory() as temp:
            paths=[Path(temp)/name for name in ['actual','reference']]
            for path in paths:
                path.mkdir();(path/'scalars.hex').write_text('N +1.0X+0\nrank +1.0X+0\n')
            record,target=[{'directory':str(p)} for p in paths]
            zero='b.1 +0.0X+0\nV.1.1 +0.0X+0\n'
            tiny='b.1 +1.0X-40\nV.1.1 +1.0X-40\n'
            for a,b in [(zero,tiny),(tiny,zero),(zero,zero)]:
                (paths[0]/'science.hex').write_text(a);(paths[1]/'science.hex').write_text(b)
                self.assertEqual(scientific_accuracy(record,target,ReferenceCache()),a==b)

    def common_captures(self, root, stage='screening', prefix='common', scopes=None, screening=None):
        def bind(path,value):
            path.write_text(json.dumps(value));return dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        history=root/'bench/history';history.mkdir(parents=True,exist_ok=True)
        (root/'coefficients.csv').write_text('name,value\nx,1\n');(root/'sample.csv').write_text('obs_id\n1\n')
        (root/'science.hex').write_text('b.1 +1.0X+0\nV.1.1 +1.0X+0\n');(root/'scalars.hex').write_text('N +1.0X+0\nrank +1.0X+0\n')
        (root/'data.dta').write_bytes(b'data');data_hash=hashlib.sha256((root/'data.dta').read_bytes()).hexdigest()
        settings=dict(model='ols',k='1',effects='0',weights='unit',vce='iid',memory='auto',output_mode='none',
            slopes='0',finite_factor='0',deterministic='0',solver='auto',threads='1',tolerance='1e-12',engine='next',arm='fereg')
        (root/'config.tsv').write_text(''.join(f'{k}\t{v}\n' for k,v in {**settings,'dataset':str(root/'data.dta')}.items()))
        (root/'metadata.tsv').write_text('complete\t1\nrc\t0\nactual_engine\tnext\n')
        paths=self.captures(root,stage,prefix,scopes,screening);plan=json.loads(paths[0].read_text())['measurement']['plan']
        plan.update(milestone='M35',manifest_sha256='common-manifest')
        artifact=plan['artifacts']['candidate']
        declaration=bind(root/'reproducible-science-declaration.json',{'artifacts':{'candidate':artifact}})
        science=bind(history/'science.json',dict(milestone='M35.6',combined_counts={'failures':0},candidate_native_sha256='p',
            source={'commit':'s'},independent_science='Independent fixture',evidence=[declaration]))
        coverage=bind(history/'coverage.json',dict(artifact=artifact,science_receipt=science,manifest_sha256='common-manifest',
            datasets={c:data_hash for c in ['a','b']},reference_settings={c:settings for c in ['a','b']},coverage_passed=True))
        plan['scientific_reference']=dict(role='candidate',science_receipt=science,coverage_receipt=coverage)
        for path in paths:
            c=json.loads(path.read_text());c.update(suite_sha256='common-manifest',datasets={x:data_hash for x in ['a','b']},
                protocol={**c['protocol'],'reference_artifact':'candidate'})
            c['records']=[r for r in c['records'] if not r['reference'] or r['artifact_role']=='candidate']
            for r in c['records']:
                if r['reference']:r.update(threads=1,engine='next',requested_solver='auto')
            c['measurement'].update(plan=plan,plan_sha256=hashlib.sha256(json.dumps(plan,sort_keys=True,separators=(',',':')).encode()).hexdigest())
            path.write_text(json.dumps(c))
        return paths,plan

    def test_common_reference_final_look_rejects_bad_evidence_and_reference_roles(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();screen,plan=self.common_captures(root)
            with patch('compare.ROOT',root):
                first=self.evaluate(screen);identity=first['parent']['blocks']+first['fixed_baseline']['blocks']
                confirm,_=self.common_captures(root,'confirmation','common-confirm',['a'],identity)
                paths=screen[:2]+confirm[:2]+screen[2:]+confirm[2:]
                result=self.evaluate(paths);self.assertTrue(result['passed'])
                self.assertEqual(result['parent']['effects'][0]['blocks'][0]['pairs'],40)
                original=screen[0].read_bytes()
                for error in ['missing','duplicate','wrong_artifact','role','non_strict','dataset']:
                    c=json.loads(original);ref=next(r for r in c['records'] if r['reference'])
                    if error=='missing':c['records'].remove(ref)
                    if error=='duplicate':c['records'].append(ref)
                    if error=='wrong_artifact':ref['plugin_sha256']='other'
                    if error=='role':ref['artifact_role']='parent'
                    if error=='non_strict':ref['threads']=8
                    if error=='dataset':c['datasets']['a']='other'
                    screen[0].write_text(json.dumps(c))
                    with self.subTest(error=error),self.assertRaises(ValueError):self.evaluate(paths)
                screen[0].write_bytes(original)
                changed=copy.deepcopy(plan);changed['artifacts']['candidate']['native_identity']['plugin_sha256']='other'
                with self.assertRaises(ValueError):validate_measurement_plan(changed)

    def test_common_reference_inherits_original_screen_and_requires_full_confirmation(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();screen,plan=self.common_captures(root)
            def bind(name,value):
                path=root/name;path.write_text(json.dumps(value))
                return dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest())
            old_suite=dict(defaults={},protocol={'reference_tolerance':1e-12},tiers={'pilot':{}},
                cells=[dict(id=c,model='ols',effects=0) for c in ['a','b']])
            old_manifest=bind('old-manifest.json',old_suite)
            new_manifest=bind('new-manifest.json',{**old_suite,'protocol':{**old_suite['protocol'],
                'reference_solver_policy':'ols-general-slopes-lsmr-v1','reference_artifact':'candidate'}})
            old=copy.deepcopy(plan);old.pop('scientific_reference')
            old.update(manifest_sha256=old_manifest['sha256'],
                command_budget=dict(screening_per_block=912,confirmation_per_block_max=4240,total_max=20608,blocks=8),
                tooling_identity={'bench/run.py':'old','bench/compare.py':'old','manifest':old_manifest['sha256']})
            originals=[]
            for path in screen[:2]:
                c=json.loads(path.read_text());ref=next(r for r in c['records'] if r['reference'])
                c['records'].insert(0,{**ref,'artifact_role':'parent'})
                # Every historical cell has its own two actual references.
                other=next(r for r in c['records'] if r['reference'] and r['cell']=='b')
                c['records'].insert(0,{**other,'artifact_role':'parent'})
                c.update(suite_sha256=old['manifest_sha256'],tooling_identity=old['tooling_identity'])
                c['protocol'].pop('reference_artifact')
                c['measurement'].update(plan=old,plan_sha256=hashlib.sha256(json.dumps(old,sort_keys=True,separators=(',',':')).encode()).hexdigest())
                path.write_text(json.dumps(c));originals.append(path.read_bytes())
            tools={**old['tooling_identity'],'bench/run.py':'new','bench/compare.py':'new',
                'manifest':new_manifest['sha256'],'scripts/fereg_f0_stata_capture.py':'decoder'}
            qrefs=[{**r,'capture_files':{f:hashlib.sha256((root/f).read_bytes()).hexdigest()
                    for f in ['coefficients.csv','sample.csv','metadata.tsv','config.tsv','science.hex','scalars.hex']}}
                for r in json.loads(screen[0].read_text())['records'] if r['reference'] and r['artifact_role']=='candidate']
            witness=bind('witness.json',dict(passed=True,captures=[dict(path=str(root),passed=True)]))
            q=bind('qualification.json',dict(manifest=new_manifest,tooling_identity=tools,artifacts=old['artifacts'],
                datasets={'a':hashlib.sha256((root/'data.dta').read_bytes()).hexdigest(),'b':hashlib.sha256((root/'data.dta').read_bytes()).hexdigest()},
                records=qrefs,independent_witness=witness,passed=True,complete=True,identities_verified=True))
            coverage=json.loads(Path(plan['scientific_reference']['coverage_receipt']['path']).read_text())
            coverage['manifest_sha256']=new_manifest['sha256']
            plan['scientific_reference']['coverage_receipt']=bind('bench/history/coverage.json',coverage)
            plan.update(manifest_sha256=new_manifest['sha256'],tooling_identity=tools,
                command_budget={**old['command_budget'],'inherited_screening_per_block':912,
                    'screening_per_block':910,'confirmation_per_block_max':4238,'total_max':20596},
                inherited_screening=dict(previous_plan=bind('old-plan.json',old),previous_manifest=old_manifest,
                    reference_qualification=q,blocks=[dict(path=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in screen[:2]]))
            def apply(paths):
                for path in paths:
                    c=json.loads(path.read_text());c.update(suite_sha256=plan['manifest_sha256'],tooling_identity=tools,
                        protocol={**c['protocol'],'reference_solver_policy':'ols-general-slopes-lsmr-v1','reference_artifact':'candidate'})
                    c['measurement'].update(plan=plan,plan_sha256=hashlib.sha256(json.dumps(plan,sort_keys=True,separators=(',',':')).encode()).hexdigest())
                    path.write_text(json.dumps(c))
            apply(screen[2:])
            with patch('compare.ROOT',root):
                first=acceptance_envelopes(screen[:2],screen[2:],'p','f',successor_plan=plan)
                identity=first['parent']['blocks']+first['fixed_baseline']['blocks']
                confirm,_=self.common_captures(root,'confirmation','inherited-common-confirm',['a'],identity)
                plan['scientific_reference']['coverage_receipt']=bind('bench/history/coverage.json',coverage)
                apply(screen[2:]+confirm)
                parent=screen[:2]+confirm[:2];fixed=screen[2:]+confirm[2:]
                final=acceptance_envelopes(parent,fixed,'p','f',successor_plan=plan)
                self.assertTrue(final['passed']);self.assertEqual(final['parent']['effects'][0]['blocks'][0]['pairs'],40)
                self.assertEqual(originals,[p.read_bytes() for p in screen[:2]])
                with self.assertRaises(ValueError):acceptance_envelopes(parent,screen[2:],'p','f',successor_plan=plan)
                raw=confirm[0].read_bytes();c=json.loads(raw);c['measurement']['screening_blocks'][0]['sha256']='wrong'
                confirm[0].write_text(json.dumps(c))
                with self.assertRaises(ValueError):acceptance_envelopes(parent,fixed,'p','f',successor_plan=plan)

    def test_strict_general_ols_reference_keeps_timed_and_explicit_requests(self):
        args = SimpleNamespace(solver='auto')
        suite = {'protocol': {'reference_solver_policy': 'ols-general-slopes-lsmr-v1'}}
        for model in ['ols', 'ppml']:
            for effects in range(5):
                cell = dict(model=model, effects=effects, slopes=True)
                self.assertEqual(command_solver(args, suite, cell, True),
                                 'lsmr' if model == 'ols' and effects >= 3 else 'auto')
                self.assertEqual(command_solver(args, suite, cell, False), 'auto')
                self.assertEqual(command_solver(args, suite, {**cell, 'slopes':False}, True), 'auto')
                for solver in ['lsmr', 'cmg', 'map']:
                    self.assertEqual(command_solver(args, suite, {**cell, 'solver': solver}, True), solver)
                    self.assertEqual(command_solver(SimpleNamespace(solver=solver), suite, cell, True), solver)
        self.assertEqual(command_solver(args, {'protocol': {}}, dict(model='ols', effects=3), True), 'auto')
        with self.assertRaises(ValueError):
            command_solver(args, {'protocol': {'reference_solver_policy': 'unknown'}}, dict(model='ols', effects=3), False)

    def test_reference_and_timed_commands_cannot_run_past_remaining_block_budget(self):
        with tempfile.TemporaryDirectory() as temp:
            args = SimpleNamespace(root=Path(temp), tier='pilot', engine='next', stata='fake-stata',
                stata_processors=4, fereg_only=True, solver='auto', measurement_plan=Path('frozen.json'),
                measurement_started=100., bounded_plan={'maximum_wall_seconds_per_block': 60.},
                native_identity={'plugin_filename': 'plugin', 'plugin_sha256': 'p',
                                 'build_source_commit': 's', 'frontend_sha256': 'f'})
            suite = dict(tiers={'pilot': {'timeout_seconds': 1800}}, engine_option=True,
                         protocol={'reference_tolerance': 1e-10})
            cell = dict(id='a', model='ols', regressors=1, effects=0, outputs=False,
                        weights='none', vce='robust')
            environment = {'references': temp, 'r_library': temp}
            timeout = dict(process_status='timeout', process_rc=None, wall_seconds=10.)
            with patch('run.time.monotonic', return_value=150.), patch('run.launch', return_value=timeout) as launched:
                for reference in [True, False]:
                    result = measure(args, suite, cell, 'fereg', 8, 1, Path(temp) / 'data.dta', environment, reference)
                    self.assertEqual(result['status'], 'timeout')
                    self.assertEqual(launched.call_args.args[2], 10.)
            with patch('run.time.monotonic', return_value=161.), patch('run.launch') as launched:
                with self.assertRaisesRegex(ValueError, 'wall budget exhausted'):
                    measure(args, suite, cell, 'fereg', 8, 2, Path(temp) / 'data.dta', environment)
                launched.assert_not_called()

    def test_failed_capture_is_checkpointed_before_the_campaign_stops(self):
        suite = dict(tiers={'pilot': {'threads': [4]}}, comparators={'ols': ['fereg']},
                     protocol={'repetitions': 5})
        args = SimpleNamespace(tier='pilot', threads_only=4, measurement_plan=Path('frozen.json'),
                               measurement_stage='screening', measurement_started=time.monotonic(),
                               bounded_plan={'maximum_wall_seconds_per_block': 1800})
        # Fail either reference, warmup, or timed arm; no following command may run.
        for failure in [0, 1, 2, 4, 5]:
            with self.subTest(failure=failure):
                records, saved = [], []
                def measure(*unused, **keywords):
                    index = len(records)
                    return dict(status='error' if index == failure else 'ok', directory=f'capture-{index}')
                with patch('run.measure', side_effect=measure) as launch:
                    with self.assertRaisesRegex(ValueError, f'capture-{failure}'):
                        execute_cell(args, [args, args], suite, dict(id='a', model='ols'), None, {},
                                     records, lambda: saved.append(copy.deepcopy(records)))
                self.assertEqual(launch.call_count, failure + 1)
                self.assertEqual(saved[-1], records)
                self.assertEqual(records[-1]['status'], 'error')

    def test_confirmation_measures_only_flagged_mixed_worker_groups(self):
        suite = dict(tiers={'pilot': {'threads': [8, 16]}}, comparators={'ols': ['fereg']},
                     protocol={'repetitions': 8})
        for flagged in [{8}, {16}, {8, 16}]:
            for reverse in [False, True]:
                args = SimpleNamespace(tier='pilot', threads_only=None,
                    measurement_plan=Path('frozen.json'), measurement_stage='confirmation',
                    measurement_started=time.monotonic(), reverse_order=reverse,
                    confirmation_groups={('a', w, 'fereg') for w in flagged},
                    bounded_plan={'maximum_wall_seconds_per_block': 1800,
                        'confirmation_pairs': {'a|8|fereg': 32, 'a|16|fereg': 64}})
                records = []
                with patch('run.measure', return_value={'status': 'ok', 'directory': 'capture'}) as launch:
                    execute_cell(args, [args, args], suite, dict(id='a', model='ols'),
                                 None, {}, records, lambda: None)
                count = {8: 66, 16: 130}
                self.assertEqual(launch.call_count, 2 + sum(count[w] for w in flagged))
                observed = [c.args[4] for c in launch.call_args_list if not c.kwargs.get('reference')]
                order = sorted(flagged, reverse=reverse)
                self.assertEqual(observed, [w for w in order for _ in range(count[w])])
                self.assertEqual([c.args[5] for c in launch.call_args_list[:2]], ['reference', 'reference'])

    def test_out_of_space_preserves_the_previous_checkpoint_and_partial_write(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'results.json'
            previous = {'records': [{'status': 'ok'}]}
            save_campaign(path, previous)
            original = path.read_bytes()
            self.assertEqual(original, (json.dumps(previous, indent=2) + '\n').encode())
            def partial_write(campaign, output, **keywords):
                output.write('{"records": [')
                raise OSError(errno.ENOSPC, 'No space left on device')
            with patch('run.json.dump', side_effect=partial_write):
                with self.assertRaises(OSError):
                    save_campaign(path, {'records': [{'status': 'error'}]})
            self.assertEqual(path.read_bytes(), original)
            self.assertEqual(path.with_name('results.json.tmp').read_text(), '{"records": [')

    def test_exact_order_statistic_endpoints_and_random_allowance(self):
        for n,k in [(8,1),(40,12),(72,25)]:
            self.assertEqual(median_order_interval(list(range(n))), (k-1-1e-6,n-k+1e-6))
        from compare import median_order_envelope
        policy=dict(slowdown_fraction=.05,short_workload_allowance_seconds=.001)
        # Bounding the parent median also bounds its relative allowance.
        result=median_order_envelope([.1]*8,[.105]*8,policy)
        self.assertEqual(result['verdict'],'unresolved')
        self.assertGreater(result['upper_excess_seconds'],0)

    def captures(self, root, stage='screening', prefix='screen', scopes=None, screening=None, regression=False):
        native={'plugin_sha256':'p','frontend_sha256':'f'}
        artifact={'source_sha':'s','native_identity':native}
        plan=dict(method='bounded-median-orders-v2',mode='acceptance',screen_pairs=8,
            confirmation_pairs={'a|4|fereg':32,'b|4|fereg':64},scope=[['a',4,'fereg'],['b',4,'fereg']],
            slowdown_fraction=.05,short_workload_allowance_seconds=.001,maximum_wall_seconds_per_block=1000,
            calibration_sha256='calibrated',artifacts={key:artifact for key in ['parent','candidate','fixed_baseline']})
        paths=[]
        for comparator in ['parent','fixed_baseline']:
            for reverse in [False,True]:
                campaign=dict(paired_artifacts={role:artifact for role in ['parent','candidate']},suite_sha256='manifest',
                    tooling_identity={'tool':'hash'},datasets={cell:'data' for cell in (scopes or ['a','b'])},
                    platform='darwin',tier='pilot',environment={'reference':'hash'},
                    timing_identity={'method':'monotonic-command-v1','plugin_sha256':'clock'},
                    protocol=dict(repetitions=8,slowdown_fraction=.05,short_workload_allowance_seconds=.001,coefficient_atol=1e-10,coefficient_rtol=1e-7),
                    complete=True,identities_verified=True,wall_seconds=10,records=[],expected_groups=[],
                    measurement=dict(plan=plan,plan_sha256=hashlib.sha256(json.dumps(plan,sort_keys=True,separators=(',',':')).encode()).hexdigest(),
                        stage=stage,comparator=comparator,screening_blocks=screening or []))
                campaign['reverse_order']=reverse
                for cell in scopes or ['a','b']:
                    for role in ['parent','candidate']:
                        campaign['records'].append(dict(cell=cell,artifact_role=role,reference=True,status='ok',directory=str(root),**native))
                        campaign['expected_groups'].append([cell,4,'fereg',role])
                    count=8 if stage=='screening' else plan['confirmation_pairs'][f'{cell}|4|fereg']
                    for repetition in ['warmup',*range(1,count+1)]:
                        roles=['parent','candidate']
                        if reverse ^ (isinstance(repetition,int) and repetition%2==0):roles.reverse()
                        for role in roles:
                            seconds=.1
                            if stage=='screening' and cell=='a' and isinstance(repetition,int):
                                seconds=.088 if repetition%2 else .112
                            if regression and role=='candidate':seconds=.13
                            campaign['records'].append(dict(cell=cell,threads=4,arm='fereg',artifact_role=role,reference=False,
                                repetition=repetition,status='ok',directory=str(root),metadata=dict(seconds=str(seconds),clock_rc='0',clock_resolution_seconds='1e-9'),
                                timing_identity=campaign['timing_identity'],**native))
                path=root/f'{prefix}-{comparator}-{reverse}.json';path.write_text(json.dumps(campaign));paths.append(path)
        return paths

    def evaluate(self, paths):
        return acceptance_envelopes(paths[:len(paths)//2],paths[len(paths)//2:],'p','f')

    def reference_successor(self, root):
        def save(path, value):
            path.write_text(json.dumps(value))
            return dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        (root/'coefficients.csv').write_text('name,value\nx,1\n')
        (root/'sample.csv').write_text('obs_id\n1\n')
        dataset=root/'data.dta';dataset.write_bytes(b'immutable data')
        data_hash=hashlib.sha256(dataset.read_bytes()).hexdigest()
        config=dict(solver='auto',threads='1',model='ols',tolerance='1e-12',dataset=str(dataset),vce='robust')
        (root/'config.tsv').write_text(''.join(f'{k}\t{v}\n' for k,v in config.items()))
        old_suite=dict(defaults={},protocol={'reference_tolerance':1e-12},
            tiers={'pilot':{}},cells=[dict(id='a',model='ols',effects=3,slopes=True),dict(id='b',model='ppml',effects=3)])
        old_manifest=save(root/'old-manifest.json',old_suite)
        paths=self.captures(root)
        old=json.loads(paths[0].read_text())['measurement']['plan']
        old.update(milestone='M35',manifest_sha256=old_manifest['sha256'],
            tooling_identity={'bench/run.py':'old-run','bench/compare.py':'old-compare','manifest':old_manifest['sha256'],'bench/generate.py':'unchanged'})
        for path in paths:
            c=json.loads(path.read_text());c.update(suite_sha256=old['manifest_sha256'],tooling_identity=old['tooling_identity'],datasets={'a':data_hash,'b':data_hash})
            c['measurement'].update(plan=old,plan_sha256=hashlib.sha256(json.dumps(old,sort_keys=True,separators=(',',':')).encode()).hexdigest())
            save(path,c)
        previous_plan=save(root/'old-plan.json',old)
        suite={**old_suite,'protocol':{**old_suite['protocol'],'reference_solver_policy':'ols-general-slopes-lsmr-v1'}}
        manifest=save(root/'new-manifest.json',suite)
        tools={**old['tooling_identity'],'bench/run.py':'new-run','bench/compare.py':'new-compare','manifest':manifest['sha256']}
        records=[]
        for role in ['parent','candidate']:
            directory=root/role;directory.mkdir()
            for filename in ['coefficients.csv','sample.csv']:(directory/filename).write_bytes((root/filename).read_bytes())
            (directory/'metadata.tsv').write_text('complete\t1\nrc\t0\nactual_engine\tnext\n')
            (directory/'config.tsv').write_text(''.join(f'{k}\t{v}\n' for k,v in {**config,'solver':'lsmr'}.items()))
            records.append(dict(cell='a',artifact_role=role,reference=True,threads=1,requested_solver='lsmr',status='ok',
                plugin_sha256='p',frontend_sha256='f',engine='next',directory=str(directory),
                capture_files={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in directory.iterdir()}))
        witness=save(root/'witness.json',dict(passed=True,captures=[dict(path=r['directory'],passed=True) for r in records]))
        qualification=save(root/'qualification.json',dict(manifest=manifest,tooling_identity=tools,artifacts=old['artifacts'],independent_witness=witness,
            datasets={'a':data_hash,'b':data_hash},records=records,passed=True,complete=True,identities_verified=True))
        successor={**old,'manifest_sha256':manifest['sha256'],'tooling_identity':tools,
            'inherited_screening':dict(previous_plan=previous_plan,previous_manifest=old_manifest,reference_qualification=qualification,
                blocks=[dict(path=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in paths[:2]])}
        for path in paths[2:]:
            c=json.loads(path.read_text());c.update(suite_sha256=successor['manifest_sha256'],tooling_identity=tools,
                protocol={**c['protocol'],'reference_solver_policy':'ols-general-slopes-lsmr-v1'})
            c['measurement'].update(plan=successor,plan_sha256=hashlib.sha256(json.dumps(successor,sort_keys=True,separators=(',',':')).encode()).hexdigest())
            save(path,c)
        return paths,successor

    def test_reference_successor_preserves_statistics_and_rejects_other_changes(self):
        from compare import bounded_pair_envelopes
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);paths,plan=self.reference_successor(root)
            original=[p.read_bytes() for p in paths[:2]]
            before=bounded_pair_envelopes(paths[:2])
            result=acceptance_envelopes(paths[:2],paths[2:],'p','f',successor_plan=plan)
            self.assertEqual(result['parent']['effects'],before['effects'])
            self.assertEqual(result['parent']['blocks'],before['blocks'])
            self.assertEqual([p.read_bytes() for p in paths[:2]],original)
            for corruption in ['budget','allowance','tool','unlisted','source','reference_hash']:
                changed=copy.deepcopy(plan)
                if corruption=='budget':changed['confirmation_pairs']['a|4|fereg']=64
                if corruption=='allowance':changed['short_workload_allowance_seconds']=.002
                if corruption=='tool':changed['tooling_identity']['bench/generate.py']='other'
                if corruption=='unlisted':changed['inherited_screening']['blocks'][0]['sha256']='other'
                if corruption=='source':changed['artifacts']['candidate']['source_sha']='other'
                if corruption=='reference_hash':changed['inherited_screening']['reference_qualification']['sha256']='other'
                with self.subTest(corruption=corruption),self.assertRaises(ValueError):
                    acceptance_envelopes(paths[:2],paths[2:],'p','f',successor_plan=changed)
            # Updating the binding cannot disguise a changed reference model or sample.
            binding=plan['inherited_screening']['reference_qualification']
            q_path=Path(binding['path']);q=json.loads(q_path.read_text())
            for filename,payload in [('config.tsv','solver\tlsmr\nthreads\t1\nmodel\tols\ntolerance\t1e-12\ndataset\t'+str(root/'data.dta')+'\nvce\tiid\n'),
                                     ('sample.csv','obs_id\n2\n'),('metadata.tsv','complete\t1\nrc\t459\nactual_engine\tnext\n')]:
                ref=q['records'][0];path=Path(ref['directory'])/filename;raw=path.read_bytes()
                path.write_text(payload);ref['capture_files'][filename]=hashlib.sha256(path.read_bytes()).hexdigest()
                q_path.write_text(json.dumps(q));binding['sha256']=hashlib.sha256(q_path.read_bytes()).hexdigest()
                with self.subTest(filename=filename),self.assertRaises(ValueError):
                    acceptance_envelopes(paths[:2],paths[2:],'p','f',successor_plan=plan)
                path.write_bytes(raw);ref['capture_files'][filename]=hashlib.sha256(raw).hexdigest()

    def test_inherited_screening_final_look_requires_both_comparators_and_provenance(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);screen,plan=self.reference_successor(root)
            first=acceptance_envelopes(screen[:2],screen[2:],'p','f',successor_plan=plan)
            self.assertEqual(confirmation_scope([first['parent'],first['fixed_baseline']]),{('a',4,'fereg')})
            identity=first['parent']['blocks']+first['fixed_baseline']['blocks']
            confirm=self.captures(root,'confirmation','inherit-confirm',['a'],identity)
            for path in confirm:
                c=json.loads(path.read_text());c.update(suite_sha256=plan['manifest_sha256'],tooling_identity=plan['tooling_identity'],
                    protocol={**c['protocol'],'reference_solver_policy':'ols-general-slopes-lsmr-v1'},datasets=first['parent']['datasets'])
                c['measurement'].update(plan=plan,plan_sha256=hashlib.sha256(json.dumps(plan,sort_keys=True,separators=(',',':')).encode()).hexdigest())
                path.write_text(json.dumps(c))
            parent=screen[:2]+confirm[:2];fixed=screen[2:]+confirm[2:]
            final=acceptance_envelopes(parent,fixed,'p','f',successor_plan=plan)
            self.assertTrue(final['passed'])
            self.assertEqual([b['pairs'] for b in final['parent']['effects'][0]['blocks']],[40,40])
            raw=confirm[0].read_bytes();c=json.loads(raw)
            c['measurement']['screening_blocks'][0]['sha256']='wrong'
            confirm[0].write_text(json.dumps(c))
            with self.assertRaises(ValueError):acceptance_envelopes(parent,fixed,'p','f',successor_plan=plan)
            confirm[0].write_bytes(raw)
            with self.assertRaises(ValueError):acceptance_envelopes(parent,screen[2:],'p','f',successor_plan=plan)

    def test_m35_waiver_is_decision_and_artifact_bound_without_a_fake_calibration(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            plan = json.loads(self.captures(root)[0].read_text())['measurement']['plan']
            plan.update(milestone='M35', calibration_sha256=None)
            with self.assertRaisesRegex(ValueError, 'prior calibrated protocol'):
                validate_measurement_plan(plan)
            directory = root / 'bench/history'
            directory.mkdir(parents=True)
            path = directory / 'decision.json'
            decision = dict(milestone='M35', authorized_by='Johannes', decision='skip_self_calibration',
                artifacts={role:dict(source_sha='s',plugin_sha256='p',frontend_sha256='f')
                           for role in plan['artifacts']})
            path.write_text(json.dumps(decision))
            plan['calibration_waiver'] = dict(decision_path='bench/history/decision.json',
                decision_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
            with patch('compare.ROOT', root):
                self.assertEqual(validate_measurement_plan(plan), [('a',4,'fereg'),('b',4,'fereg')])
                for corruption in ['hash','path','artifact','milestone','mode','fake_pass','malformed']:
                    changed = copy.deepcopy(plan)
                    if corruption == 'hash': changed['calibration_waiver']['decision_sha256'] = 'other'
                    if corruption == 'path': changed['calibration_waiver']['decision_path'] = '../decision.json'
                    if corruption == 'artifact': changed['artifacts']['parent']['native_identity']['plugin_sha256'] = 'other'
                    if corruption == 'milestone': changed['milestone'] = 'M36'
                    if corruption == 'mode': changed['mode'] = 'calibration'
                    if corruption == 'fake_pass': changed['calibration_sha256'] = 'passed'
                    if corruption == 'malformed': changed['calibration_waiver'] = {'reason':'skip'}
                    with self.subTest(corruption=corruption), self.assertRaises(ValueError):
                        validate_measurement_plan(changed)
                for field,value in [('authorized_by','other'),('decision','passed'),('milestone','M36')]:
                    changed_decision = {**decision,field:value}
                    path.write_text(json.dumps(changed_decision))
                    changed = copy.deepcopy(plan)
                    changed['calibration_waiver']['decision_sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
                    with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'authorized M35 scope'):
                        validate_measurement_plan(changed)

    def test_targeted_confirmation_retains_screen_and_both_comparators(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            (root/'coefficients.csv').write_text('name,value\nx,1\n');(root/'sample.csv').write_text('obs_id\n1\n')
            screen=self.captures(root)
            result=self.evaluate(screen)
            self.assertFalse(result['passed'])
            self.assertEqual([e['verdict'] for e in result['parent']['effects']],['unresolved','within'])
            identity=result['parent']['blocks']+result['fixed_baseline']['blocks']
            confirm=self.captures(root,'confirmation','confirm',['a'],identity)
            paths=screen[:2]+confirm[:2]+screen[2:]+confirm[2:]
            result=self.evaluate(paths)
            self.assertTrue(result['passed'])
            self.assertEqual([b['pairs'] for b in result['parent']['effects'][0]['blocks']],[40,40])
            original=confirm[0].read_text()
            for corruption in ['omission','clock','plan','ref','screen','extra_scope','budget']:
                data=json.loads(original)
                if corruption=='omission':data['records'].pop()
                if corruption=='clock':data['records'][-1]['metadata']['clock_rc']='1'
                if corruption=='plan':data['measurement']['plan']['confirmation_pairs']['a|4|fereg']=64
                if corruption=='ref':data['records'][0]['plugin_sha256']='other'
                if corruption=='screen':data['measurement']['screening_blocks'][0]['sha256']='other'
                if corruption=='extra_scope':data['expected_groups'].append(['b',4,'fereg','parent'])
                if corruption=='budget':data['wall_seconds']=1001
                confirm[0].write_text(json.dumps(data))
                with self.assertRaises(ValueError,msg=corruption):self.evaluate(paths)
            confirm[0].write_text(original)

    def test_regression_is_distinct_from_screening_uncertainty(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            (root/'coefficients.csv').write_text('name,value\nx,1\n');(root/'sample.csv').write_text('obs_id\n1\n')
            screen=self.captures(root,regression=True);result=self.evaluate(screen)
            self.assertFalse(result['passed']);self.assertTrue(all(e['verdict']=='unresolved' for e in result['parent']['effects']))
            identity=result['parent']['blocks']+result['fixed_baseline']['blocks']
            confirm=self.captures(root,'confirmation','confirm',['a','b'],identity,True)
            result=self.evaluate(screen[:2]+confirm[:2]+screen[2:]+confirm[2:])
            self.assertFalse(result['passed']);self.assertTrue(all(e['verdict']=='outside' for e in result['parent']['effects']))
            for e in result['parent']['effects']:
                self.assertEqual(e['blocks'][0]['pairs'],40 if e['cell']=='a' else 72)

    def test_same_thread_artifact_pass_cannot_hide_worker_regression(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'coefficients.csv').write_text('name,value\nx,1\n')
            (root / 'sample.csv').write_text('obs_id\n1\n')

            def worker_captures(stage, prefix, screening=None):
                paths = self.captures(root, stage, prefix, screening=screening)
                for path in paths:
                    data = json.loads(path.read_text())
                    plan = data['measurement']['plan']
                    plan.update(scope=[['a', 8, 'fereg'], ['a', 16, 'fereg']],
                                confirmation_pairs={'a|8|fereg': 32, 'a|16|fereg': 64},
                                scaling_edges=[['a', 8, 16, 'fereg']])
                    data['measurement']['plan_sha256'] = hashlib.sha256(
                        json.dumps(plan, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
                    data['datasets'] = {'a': 'data'}
                    data['records'] = [r for r in data['records'] if not (r['reference'] and r['cell'] == 'b')]
                    for record in data['records']:
                        if not record['reference']:
                            record['threads'] = 8 if record['cell'] == 'a' else 16
                            record['cell'] = 'a'
                            record['metadata']['seconds'] = str(.1 if record['threads'] == 8 else .13)
                    data['expected_groups'] = [['a', t, 'fereg', role]
                                               for t in [8, 16] for role in ['parent', 'candidate']]
                    path.write_text(json.dumps(data))
                return paths

            screen = worker_captures('screening', 'workers-screen')
            result = self.evaluate(screen)
            self.assertFalse(result['passed'])
            self.assertTrue(all(e['verdict'] == 'within' for e in result['parent']['effects']))
            self.assertEqual(confirmation_scope([result['parent'], result['fixed_baseline']]),
                             {('a', 8, 'fereg'), ('a', 16, 'fereg')})
            identity = result['parent']['blocks'] + result['fixed_baseline']['blocks']
            confirm = worker_captures('confirmation', 'workers-confirm', identity)
            result = self.evaluate(screen[:2] + confirm[:2] + screen[2:] + confirm[2:])
            for comparison in ['parent', 'fixed_baseline']:
                self.assertTrue(all(e['verdict'] == 'within' for e in result[comparison]['effects']))
                self.assertEqual(result[comparison]['scaling_effects'][0]['verdict'], 'outside')
            for path in confirm[:2]:
                data = json.loads(path.read_text())
                data['records'] = [r for r in data['records'] if r['reference'] or r['threads'] == 8]
                data['expected_groups'] = [g for g in data['expected_groups'] if g[1] == 8]
                path.write_text(json.dumps(data))
            with self.assertRaisesRegex(ValueError, 'endpoint'):
                self.evaluate(screen[:2] + confirm[:2] + screen[2:] + confirm[2:])

    def test_scaling_edges_cannot_change_cell_or_escape_frozen_scope(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            plan = json.loads(self.captures(root)[0].read_text())['measurement']['plan']
            for edges in [[['a', 4, 16, 'fereg']], [['a', 4, 4, 'fereg']],
                          [['a', 16, 4, 'fereg']], [['a', 4, 16, 'fereg']] * 2]:
                with self.subTest(edges=edges), self.assertRaises(ValueError):
                    validate_measurement_plan({**plan, 'scaling_edges': edges})

    def test_artifact_flag_closes_connected_otherwise_settled_worker_edges(self):
        comparison = dict(effects=[dict(cell='a', threads=7, arm='fereg',
                                       screening_blocks=[{'verdict': 'unresolved'}])],
            scaling_effects=[dict(cell=cell, low_threads=low, high_threads=high, arm='fereg',
                                 screening_blocks=[{'verdict': 'within'}])
                            for cell, low, high in [('a', 7, 8), ('a', 8, 9), ('b', 8, 16)]])
        self.assertEqual(confirmation_scope([comparison]),
                         {('a', threads, 'fereg') for threads in [7, 8, 9]})


if __name__=='__main__':unittest.main()
