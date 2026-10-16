import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import run_scc as runner


class RunnerTests(unittest.TestCase):
    def test_scheduler_rejects_other_job_and_missing_allocation(self):
        def raw(ids):
            return '<job_info>' + ''.join(
                f'<job_list><JB_job_number>{i}</JB_job_number><queue_name>batch@scc-test</queue_name><slots>28</slots></job_list>'
                for i in ids) + '</job_info>'
        with patch.dict(runner.os.environ, {'JOB_ID': '123'}):
            for ids in [[], ['123', '456']]:
                with patch.object(runner, 'command', side_effect=['scc-test\n', raw(ids)]):
                    with self.assertRaises(RuntimeError):
                        runner.scheduler_snapshot()
            with patch.object(runner, 'command', side_effect=['scc-test\n', raw(['123'])]):
                self.assertEqual(runner.scheduler_snapshot()['jobs'][0]['id'], '123')

    def test_timeout_is_a_retained_failure(self):
        with patch.object(runner.subprocess, 'run', side_effect=subprocess.TimeoutExpired('solve', 30)):
            self.assertEqual(runner.invoke(Path('/binary'), 'grid', 64, 1, 'full')['status'], 'timeout')

    def test_wrong_result_identity_and_nan_fail(self):
        good = dict(status='ok', case='grid', n=64, rhs=1, arm='full', seconds=1,
                    relative_residual=1e-10, graph_builds=1)
        for change in [dict(case='hubs'), dict(seconds=float('nan')), dict(relative_residual=1e-5)]:
            result = subprocess.CompletedProcess([], 0, json.dumps({**good, **change}), '')
            with patch.object(runner.subprocess, 'run', return_value=result):
                self.assertEqual(runner.invoke(Path('/binary'), 'grid', 64, 1, 'full')['status'], 'failed')

    def test_cpu_time_and_padded_process_paths(self):
        self.assertEqual(runner.cpu_seconds('1-02:03:04'), 93784)
        with tempfile.TemporaryDirectory() as directory:
            observer = runner.Observer(Path(directory) / 'host.jsonl')
            observer.previous = {99: (1, 0, 'python3')}
            with patch.object(runner, 'command', return_value='99 1 00:00:03   /usr/bin/python3\n'), \
                    patch.object(runner, 'scheduler_snapshot', return_value={}):
                with self.assertRaises(RuntimeError):
                    observer.sample()

    def test_partial_affinity_is_not_whole_node_evidence(self):
        with patch.dict(runner.os.environ, {'NSLOTS': '28', 'PE': 'omp28'}), \
                patch.object(runner, 'command', side_effect=[
                    'scc-test\n', 'allocation_rule $pe_slots',
                    'enabled TRUE\nlimit hosts {*} to slots=$num_proc',
                    'processors 28\ncomplex_values num_proc=28',
                    'num_proc np INT == YES NO 0 0\nslots s INT <= YES YES 1 0',
                    '\n'.join(f'{i},{i},0,Y' for i in range(28))]), \
                patch.object(runner.os, 'sched_getaffinity', return_value=set(range(14)), create=True):
            with self.assertRaises(AssertionError):
                runner.configuration()

    def test_summary_does_not_pass_incomplete_or_failed_blocks(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'rows.jsonl'
            out = Path(directory) / 'summary.json'
            for block in range(6):
                for arm in ['diagonal', 'full', 'forest']:
                    for _ in range(2):
                        result = dict(status='ok', seconds=2 if arm == 'diagonal' else 1,
                            setup_seconds=.1, iterations=10, upgrades=0, digest='abc')
                        if arm == 'forest' and block == 3:
                            result = {'status': 'timeout'}
                        runner.append(path, dict(case='grid', rhs_count=1, rhs_kind='random', phase='measure',
                                                block=block, arm=arm, result=result))
            runner.summarize(path, out)
            cell = [r for r in json.loads(out.read_text())['results']
                    if r['case'] == 'grid' and r['rhs_kind'] == 'random']
            self.assertEqual(next(r for r in cell if r['arm'] == 'full')['median_ratio'], .5)
            self.assertIsNone(next(r for r in cell if r['arm'] == 'forest')['median_ratio'])
            self.assertIsNone(next(r for r in cell if r['arm'] == 'shallow')['median_ratio'])

    def test_protected_protocol_completeness_and_timing_free_signature(self):
        self.assertEqual(len(runner.PROTECTED_CELLS)*len(runner.PROTECTED_ARMS)*13,3718)
        self.assertEqual(len(set(runner.PROTECTED_CELLS)),22)
        a=dict(status='ok',digest='abc',iterations=3,upgrades=0,certificates=2,
               relative_residual=1e-9,graph_builds=0,coarse_n=0,tail_levels=0,
               decisions=[dict(incumbent_work=4,optional_work=1,build_active_seconds=.1)])
        b={**a,'decisions':[dict(incumbent_work=4,optional_work=1,build_active_seconds=.9)]}
        self.assertEqual(runner.signature(a),runner.signature(b))
        b['decisions'][0]['optional_work']=2
        self.assertNotEqual(runner.signature(a),runner.signature(b))

    def test_legacy_reference_has_all_frozen_cells(self):
        ref=json.loads(Path(runner.__file__).with_name('discovery.json').read_text())['discovery']['results']
        self.assertEqual(len(ref),120)
        keys={(r['case'],r['rhs'],r['rhs_kind'],r['arm']) for r in ref}
        for case,n,rhs,kind in runner.ORIGINAL_CELLS:
            for arm in runner.ARMS:
                self.assertIn((case,rhs,kind,arm),keys)

    def test_manifest_rejects_invalid_grid_and_keeps_historical_cells(self):
        runner.validate_manifest(runner.ORIGINAL_CELLS,runner.ARMS)
        runner.validate_manifest(runner.SUPPLEMENT_CELLS,runner.PROTECTED_ARMS)
        runner.validate_manifest([('mosaic',8192,4,'mixed')],runner.PROTECTED_ARMS)
        with self.assertRaises(AssertionError):
            runner.validate_manifest(runner.PROTECTED_CELLS,runner.PROTECTED_ARMS)
        with self.assertRaises(AssertionError):
            runner.validate_manifest([('mosaic',256,4,'mixed')],runner.PROTECTED_ARMS)
        self.assertIn(('grid',8192,4,'mixed'),runner.PROTECTED_CELLS)
        self.assertEqual(len(runner.SUPPLEMENT_CELLS)*len(runner.PROTECTED_ARMS)*13,169)

    def test_persistent_manifest_and_previous_binary_identity(self):
        arms,cells,seed,budget=runner.protocol_config('persistent')
        runner.validate_manifest(cells,arms)
        self.assertEqual(len(arms),25)
        self.assertEqual(len(cells)*len(arms)*13,2600)
        self.assertEqual((seed,budget),(20261019,1800))
        good=dict(status='ok',case='grid',n=64,rhs=1,arm='full',seconds=1,relative_residual=1e-10,graph_builds=1)
        with patch.dict(runner.os.environ,{'STAGED_PREVIOUS_BINARY':'/prior'}),patch.object(runner.subprocess,'run',return_value=subprocess.CompletedProcess([],0,json.dumps(good),'')) as run:
            self.assertEqual(runner.invoke(Path('/candidate'),'grid',64,1,'previous-full')['status'],'ok')
            self.assertEqual(run.call_args.args[0][0],'/prior')


    def test_preflight_checks_budget_and_observer_before_each_solve(self):
        with tempfile.TemporaryDirectory() as directory,patch.object(runner,'invoke',return_value={'status':'timeout'}) as invoke:
            check=unittest.mock.Mock(side_effect=[None,RuntimeError('budget or isolation')])
            with self.assertRaises(RuntimeError):
                runner.preflight(Path('/binary'),Path(directory),[('grid',64,1,'cut')],['full','diagonal'],0,check)
            self.assertEqual(invoke.call_count,1)
            self.assertEqual(check.call_count,2)
            self.assertEqual(len((Path(directory)/'preflight.jsonl').read_text().splitlines()),1)


class RouterProtocol(unittest.TestCase):
    def test_admitted_protocol_and_hub_required(self):
        import evaluate_router
        arms,cells,seed,budget=runner.protocol_config('admitted-router')
        runner.validate_manifest(cells,arms)
        self.assertEqual((len(arms),len(cells),seed,budget),(5,14,20261022,1800))
        arms,cells,seed,budget=runner.protocol_config('admitted-gate')
        runner.validate_manifest(cells,arms)
        self.assertEqual((len(arms),len(cells),seed,budget),(2,5,20261023,300))
        rows=[];pre=[]
        for case,n,rhs,kind in cells:
            for arm in arms:
                result=dict(status='ok',digest='same',iterations=1,upgrades=0,certificates=2,relative_residual=1e-10,graph_builds=1,coarse_n=0,tail_levels=0,decisions=[],seconds=.1,setup_seconds=.01)
                r=dict(case=case,n=n,rhs_count=rhs,rhs_kind=kind,arm=arm,result=result)
                pre.append(r)
                rows.extend([{**r,'phase':'warmup','block':-1}]+[{**r,'phase':'measure','block':b} for b in range(6) for _ in range(2)])
        with tempfile.TemporaryDirectory() as directory:
            out=Path(directory)
            def write():
                for filename,data in [('observations.jsonl',rows),('preflight.jsonl',pre)]:
                    (out/filename).write_text(''.join(json.dumps(r)+'\n' for r in data))
            write()
            report=evaluate_router.constructor_gate(out,admitted=True)
            self.assertTrue(report['passed'])
            self.assertTrue(all(c['gate_required'] for c in report['cells']))
            saved=pre[-1];pre[-1]=pre[0];write()
            with self.assertRaises(AssertionError):evaluate_router.constructor_gate(out,admitted=True)
            pre[-1]=saved
            saved=rows[13];rows[13]=rows[0];write()
            with self.assertRaises(AssertionError):evaluate_router.constructor_gate(out,admitted=True)
            rows[13]=saved
            rows[1]={**rows[1],'phase':'warmup'};write()
            with self.assertRaises(AssertionError):evaluate_router.constructor_gate(out,admitted=True)
            rows[1]={**rows[1],'phase':'measure'}
            next(r['result'] for r in pre if r['case']=='hubs' and r['arm']=='full-admitted').update(status='failed',reason='memory_refused')
            write()
            with self.assertRaises(AssertionError):evaluate_router.constructor_gate(out,admitted=True)

    def test_router_manifest_and_gate_counts(self):
        arms,cells,seed,budget=runner.protocol_config('router')
        runner.validate_manifest(cells,arms)
        self.assertEqual((len(arms),len(cells),seed),(5,14,20261021))
        self.assertEqual(len(arms)*len(cells)*13,910)
        self.assertEqual(len(runner.GUARDED_ARMS)*len(runner.GUARDED_CELLS)*13,130)
        self.assertIn(('mosaic',8192,4,'components-rev'),cells)

    def test_gate_rejects_setup_science_and_completeness(self):
        import evaluate_router
        rows=[];pre=[]
        for case,n,rhs,kind in runner.GUARDED_CELLS:
            for arm in runner.GUARDED_ARMS:
                result=dict(status='ok',digest='same',iterations=1,upgrades=0,certificates=2,relative_residual=1e-10,graph_builds=1,coarse_n=0,tail_levels=0,decisions=[],seconds=.1,setup_seconds=.01)
                r=dict(case=case,n=n,rhs_count=rhs,rhs_kind=kind,arm=arm,result=result)
                pre.append(r)
                rows.extend([{**r,'phase':'warmup','block':-1}]+[{**r,'phase':'measure','block':b} for b in range(6) for _ in range(2)])
        with tempfile.TemporaryDirectory() as directory:
            out=Path(directory)
            def write():
                for filename,data in [('observations.jsonl',rows),('preflight.jsonl',pre)]:
                    (out/filename).write_text(''.join(json.dumps(r)+'\n' for r in data))
            write();self.assertTrue(evaluate_router.constructor_gate(out)['passed'])
            selected=next(r for r in rows if r['case']=='grid' and r['arm']=='full-guarded')['result']
            selected['setup_seconds']=.04
            write();self.assertFalse(evaluate_router.constructor_gate(out)['passed'])
            selected['digest']='different';write()
            with self.assertRaises(AssertionError):evaluate_router.constructor_gate(out)
            rows.pop();write()
            with self.assertRaises(AssertionError):evaluate_router.constructor_gate(out)

    def test_selector_retains_failures_and_rejects_incomplete_and_mismatch(self):
        import evaluate_router
        rows = []; pre = []
        for case,n,rhs,kind in runner.ROUTER_CELLS:
            for arm in runner.ROUTER_ARMS:
                result = dict(status='ok', digest=arm, iterations=1, upgrades=0, certificates=2,
                              relative_residual=1e-10, graph_builds=1, coarse_n=0, tail_levels=0,
                              decisions=[], seconds=.1)
                r = dict(case=case,n=n,rhs_count=rhs,rhs_kind=kind,arm=arm,result=result)
                pre.append(r)
                rows.extend([{**r,'phase':'warmup','block':-1}] + [{**r,'phase':'measure','block':b} for b in range(6) for _ in range(2)])
        with tempfile.TemporaryDirectory() as directory:
            out=Path(directory)
            def write():
                for filename,data in [('observations.jsonl',rows),('preflight.jsonl',pre)]:
                    (out/filename).write_text(''.join(json.dumps(r)+'\n' for r in data))
            write(); report=evaluate_router.selector_report(out)
            self.assertTrue(report['passed'])
            self.assertEqual(report['cells'][3]['comparator'],'diagonal')
            selected = next(r for r in pre if r['case']=='grid' and r['n']==8100 and r['arm']=='auto-commit')['result']
            selected.update(status='failed',reason='numerical_limit')
            write(); report=evaluate_router.selector_report(out)
            cell=next(c for c in report['cells'] if c['case']=='grid' and c['n']==8100)
            self.assertFalse(cell['passed']); self.assertEqual(cell['arms']['auto-commit']['failures'],14)
            rows[-1] = {**rows[-1], 'result': {**rows[-1]['result'], 'digest':'changed'}}
            write()
            with self.assertRaises(AssertionError): evaluate_router.selector_report(out)
            rows.pop();write()
            with self.assertRaises(AssertionError): evaluate_router.selector_report(out)


if __name__ == '__main__':
    unittest.main()
