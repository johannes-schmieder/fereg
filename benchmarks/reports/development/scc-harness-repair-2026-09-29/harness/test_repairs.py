import hashlib
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE/'cg'), str(HERE/'comparator'), str(HERE/'public')]
from prepare_continuation import pending_plan
from test_inputs import stage_test_inputs
from gtools_check import fixture_passed
spec = importlib.util.spec_from_file_location('public_run', HERE/'public/run.py')
public = importlib.util.module_from_spec(spec)
spec.loader.exec_module(public)


class Repairs(unittest.TestCase):
    def test_registered_fixture_is_required_and_hash_bound(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);stage=root/'stage';source=root/'source'
            stage.mkdir();source.mkdir()
            module=source/'crates/fereg-core/src/lib.rs';module.parent.mkdir(parents=True)
            module.write_text('const S: &str = include_str!("../../../fixture.csv");')
            fixture=stage/'fixture.csv';fixture.write_text('known input')
            inputs={'fixture.csv':hashlib.sha256(fixture.read_bytes()).hexdigest()}
            stage_test_inputs(stage,source,inputs)
            self.assertEqual((source/'fixture.csv').read_text(),'known input')
            fixture.write_text('different')
            with self.assertRaises(ValueError):stage_test_inputs(stage,source,inputs)
            fixture.unlink()
            with self.assertRaises(FileNotFoundError):stage_test_inputs(stage,source,inputs)

    def test_unstaged_or_external_include_fails_before_build(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);module=root/'crates/fereg-core/src/lib.rs';module.parent.mkdir(parents=True)
            for name in ['../../../missing.csv','../../../../outside.csv']:
                module.write_text('include_bytes!("'+name+'");')
                with self.assertRaises(ValueError):stage_test_inputs(root,root,{})

    def gtools(self, folder):
        (folder/'gtools.log').write_text('GTOOLS_SAMPLE_FIXTURE_COMPLETE\n')
        (folder/'checks.tsv').write_text('case\tN\tsample_exact\tb_V_unchanged\n1\t600\t1\t1\n2\t585\t1\t1\n3\t584\t1\t1\n4\t585\t1\t1\nempty_sample\t0\tno_e_b\tnot_ranked\n')

    def test_actual_gtools_log_and_all_rows_required(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp);self.gtools(folder)
            self.assertTrue(fixture_passed(folder,0))
            self.assertFalse(fixture_passed(folder,1))
            log=folder/'gtools.log';log.rename(folder/'gtools_sample.log')
            self.assertFalse(fixture_passed(folder,0))
            self.gtools(folder)
            for text in ['','GTOOLS_SAMPLE_FIXTURE_COMPLETE\n'*2,'r(9);\nGTOOLS_SAMPLE_FIXTURE_COMPLETE\n']:
                log.write_text(text);self.assertFalse(fixture_passed(folder,0))
            self.gtools(folder)
            checks=folder/'checks.tsv';checks.write_text(checks.read_text().replace('3\t584\t1\t1','3\t584\t0\t1'))
            self.assertFalse(fixture_passed(folder,0))
            self.gtools(folder);checks.write_text(checks.read_text().replace('4\t585\t1\t1\n',''))
            self.assertFalse(fixture_passed(folder,0))

    def test_continuation_never_changes_or_repeats_groups(self):
        import copy
        group=dict(cell=dict(model='ppml',n=48),repetition=1,predicted_seconds=2,limits=dict(native=dict(command=120)))
        other=dict(copy.deepcopy(group),repetition=2)
        original=dict(bundles=[dict(groups=[group,other])])
        receipts=[dict(status='checkpointed',completed=[group],pending=[other])]
        key=(str(sorted(group['cell'].items())),1)
        plan=pending_plan(original,receipts,{key})
        self.assertEqual(plan['bundles'][0]['groups'],[other])
        bad=copy.deepcopy(receipts);bad[0]['pending'][0]['limits']['native']['command']=240
        with self.assertRaises(ValueError):pending_plan(original,bad,{key})
        with self.assertRaises(ValueError):pending_plan(original,receipts,{key,(key[0],2)})
        with self.assertRaises(ValueError):pending_plan(original,receipts,set())
        with self.assertRaises(ValueError):pending_plan(original,[],{key})

    def accounting(self, exit_status=0):
        return 'hostname node\njobnumber 10\ntaskid undefined\nfailed 0\nexit_status '+str(exit_status)+'\nslots 8\nru_wallclock 1\nmaxvmem 1G\n'

    def test_accounting_timeout_then_clean_receipt(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'attempts.json'
            sequence=[subprocess.TimeoutExpired('qacct',60),subprocess.CompletedProcess([],0,self.accounting(),'')]
            with patch.object(public.subprocess,'run',side_effect=sequence) as run, patch.object(public.time,'sleep'):
                result=public.query_accounting('10',path,time.monotonic()+300)
            self.assertEqual(result['status'],'pass');self.assertEqual(run.call_count,2)
            self.assertIn('query_timeout',path.read_text())

    def test_exhausted_queries_remain_pending_and_bad_accounting_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'attempts.json'
            with patch.object(public.subprocess,'run',side_effect=subprocess.TimeoutExpired('qacct',60)) as run, patch.object(public.time,'sleep'):
                self.assertEqual(public.query_accounting('10',path,time.monotonic()+300)['status'],'pending')
            self.assertEqual(run.call_count,3)
            with patch.object(public.subprocess,'run',return_value=subprocess.CompletedProcess([],0,self.accounting(1),'')) as run:
                self.assertEqual(public.query_accounting('10',path,time.monotonic()+300)['status'],'failed')
            self.assertEqual(run.call_count,1)
            with patch.object(public.subprocess,'run') as run:
                self.assertEqual(public.query_accounting('10',path,time.monotonic()-1)['status'],'pending')
                run.assert_not_called()


if __name__=='__main__':unittest.main()
