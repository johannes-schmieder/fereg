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
from compare import acceptance_envelopes, median_order_interval, validate_measurement_plan
from run import execute_cell, save_campaign


class BoundedMeasurementTests(unittest.TestCase):
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


if __name__=='__main__':unittest.main()
