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
from compare import acceptance_envelopes, confirmation_scope, median_order_interval, validate_measurement_plan
from run import execute_cell, measure, save_campaign


class BoundedMeasurementTests(unittest.TestCase):
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
