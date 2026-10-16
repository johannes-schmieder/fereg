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


if __name__ == '__main__':
    unittest.main()
