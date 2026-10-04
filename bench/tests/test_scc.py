"""SCC collection must reject incomplete accounting and benchmark coverage."""
import importlib.util
import csv
import json
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from run import load_suite, source_identity, selected_threads, selected_cells
spec=importlib.util.spec_from_file_location('scc_collect',Path(__file__).resolve().parents[1]/'scc/collect.py')
collect=importlib.util.module_from_spec(spec)
spec.loader.exec_module(collect)


class SccTests(unittest.TestCase):
    def test_accounting_requires_every_array_task_and_clean_exit(self):
        def record(task,failed=0,exit_status=0):
            return f'=====\ntaskid {task}\nfailed {failed}\nexit_status {exit_status}\nslots 28\n'
        good=record(1)+record(2)
        self.assertEqual(len(collect.accounting(good,2)),2)
        for bad in [record(1),record(1)+record(1),record(1)+record(2,exit_status=1),record(1)+record(2,failed=100)]:
            with self.assertRaises(ValueError):
                collect.accounting(bad,2)

    def test_full_task_partitions_equal_the_declared_suite(self):
        suite=load_suite()
        for tier in ['smoke','standard','full']:
            expected=collect.expected_groups(suite,tier)
            union=set()
            for cell in selected_cells(suite,tier):
                for threads in selected_threads(suite,tier,cell,None):
                    part=collect.expected_groups(suite,tier,cell['id'],str(threads))
                    self.assertFalse(union & part)
                    union |= part
            self.assertEqual(union,expected)
        with self.assertRaises(ValueError):
            collect.expected_groups(suite,'smoke','veneto-akm','28')

    def test_snapshot_identity_does_not_require_a_git_checkout(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            source={'commit':'a'*40,'archive_sha256':'b'*64}
            (root/'.fereg-source.json').write_text(json.dumps(source))
            self.assertEqual(source_identity(root),('a'*40,False))
            source['archive_sha256']='bad'
            (root/'.fereg-source.json').write_text(json.dumps(source))
            with self.assertRaises(ValueError):
                source_identity(root)

    def test_program_ladders_partition_the_complete_cube(self):
        suite=load_suite(Path(__file__).resolve().parents[1]/'program-v1.json')
        tier='development-scc'
        union=set()
        for cell in selected_cells(suite,tier):
            part=collect.expected_groups(suite,tier,cell['id'],'all')
            self.assertFalse(union & part)
            union |= part
        self.assertEqual(union,collect.expected_groups(suite,tier))
        self.assertEqual({t for _,t,_ in union},{'1','2','4','8','14','28'})

    def test_collector_rejects_missing_groups_and_wrong_source(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            (root/'summaries').mkdir()
            (root/'jobs.tsv').write_text('setup\t10\t1\nbenchmark\t11\t1\n')
            for phase in ['setup','benchmark']:
                (root/f'{phase}-qacct.txt').write_text('=====\nfailed 0\nexit_status 0\nslots 28\ntaskid 1\n')
            (root/'tasks.tsv').write_text('all all\n')
            (root/'source.json').write_text(json.dumps({'commit':'a'*40}))
            (root/'suite.toml').write_text((Path(__file__).resolve().parents[1]/'suite.toml').read_text())
            (root/'completion.txt').write_text('FEREG_BENCH_COMPLETE 16 groups 0 command failures\n')
            rows=[{'source_sha':'a'*40,'dirty':'False','tier':'smoke','platform':'linux',
                   'engine':'legacy','solver':'auto','cell':cell,'threads':thread,'arm':arm,
                   'status':'ok','accuracy':'pass','repetitions':'5','partial':'False'}
                  for cell,thread,arm in sorted(collect.expected_groups(load_suite(),'smoke'))]
            def write(part):
                with (root/'summaries/task-1.csv').open('w') as stream:
                    writer=csv.DictWriter(stream,fieldnames=list(rows[0]))
                    writer.writeheader()
                    writer.writerows(part)
            write(rows)
            self.assertEqual(len(collect.combine(root).splitlines()),17)
            write(rows[:-1])
            with self.assertRaises(ValueError):
                collect.combine(root)
            write(rows)
            (root/'source.json').write_text(json.dumps({'commit':'b'*40}))
            with self.assertRaises(ValueError):
                collect.combine(root)

    def test_thread_subset_cannot_introduce_an_undeclared_route(self):
        suite=load_suite()
        cell=selected_cells(suite,'smoke')[0]
        self.assertEqual(selected_threads(suite,'smoke',cell,4),[4])
        with self.assertRaises(ValueError):
            selected_threads(suite,'smoke',cell,28)

    def test_program_collection_binds_every_artifact_identity_to_setup(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'summaries').mkdir()
            (root/'jobs.tsv').write_text('setup\t10\t1\nbenchmark\t11\t1\n')
            for phase in ['setup','benchmark']:
                (root/f'{phase}-qacct.txt').write_text('=====\nfailed 0\nexit_status 0\nslots 28\ntaskid 1\n')
            (root/'completion.txt').write_text('FEREG_BENCH_COMPLETE 1 groups 0 command failures\n')
            source={'commit':'a'*40};(root/'source.json').write_text(json.dumps(source))
            suite=load_suite(Path(__file__).resolve().parents[1]/'program-v1.json')
            cell=suite['tiers']['baseline-pilot']['cells'][0]
            suite['tiers']['baseline-pilot']['cells']=[cell]
            suite['tiers']['baseline-pilot']['threads']=[1]
            (root/'suite.json').write_text(json.dumps(suite));(root/'tasks.tsv').write_text(f'{cell} all\n')
            manifest=hashlib.sha256((root/'suite.json').read_bytes()).hexdigest()
            tooling={'manifest':manifest,'runner':'hash'}
            (root/'policy.json').write_text(json.dumps({'engine':'next'}))
            (root/'build.json').write_text(json.dumps({'source':source,'native_identity':{'plugin_sha256':'p','frontend_sha256':'f'},'tooling_identity':tooling,'timing_identity':{'method':'monotonic-command-v1','plugin_sha256':'clock'}}))
            selected=next(c for c in selected_cells(suite,'baseline-pilot') if c['id']==cell)
            row=dict(source_sha=source['commit'],dirty='False',tier='baseline-pilot',platform='linux',engine='next',solver='auto',
                cell=cell,threads='1',arm='fereg',status='ok',accuracy='pass',repetitions='32',partial='False',memory=selected['memory'],
                output_mode=selected['output_mode'],requested_solver=selected['solver'],plugin_sha256='p',frontend_sha256='f',
                manifest_sha256=manifest,tooling_sha256=hashlib.sha256(json.dumps(tooling,sort_keys=True).encode()).hexdigest(),timing_method='monotonic-command-v1',clock_sha256='clock')
            def write(r):
                with (root/'summaries/task-1.csv').open('w') as stream:
                    writer=csv.DictWriter(stream,fieldnames=list(r));writer.writeheader();writer.writerow(r)
            write(row);self.assertEqual(len(collect.combine(root).splitlines()),2)
            for key in ['plugin_sha256','frontend_sha256','manifest_sha256','tooling_sha256','clock_sha256','timing_method']:
                write({**row,key:'wrong'})
                with self.assertRaisesRegex(ValueError,'identity'):collect.combine(root)


if __name__=='__main__':
    unittest.main()
