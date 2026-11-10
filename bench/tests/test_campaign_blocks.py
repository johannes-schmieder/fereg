"""Frozen block coverage, timeout independence and compact evidence retention."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import pandas as pd
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import campaign as c

REVISION=json.loads((c.HERE/'campaign_revision.json').read_text())


def source(**updates):
    cell=dict(id='ols-panel',model='ols',design='akm-baseline',rows=1200,threads=4,
              vce='robust',save_fe=False,regressors=7,effects=2,section='baseline',
              repetitions=3,arms=['fereg','fereg-strict','fixest','fixest-strict','areg','areg-strict'])
    cell.update(updates)
    return cell


class BlockTests(unittest.TestCase):
    def setUp(self):
        self.policy=patch.dict(c.POLICY,REVISION)
        self.policy.start()
        self.addCleanup(self.policy.stop)

    def test_preserves_each_comparator_position_and_declares_native_duplication(self):
        cells=[source(),source(id='large',rows=10_000_000,repetitions=2)]
        blocks=c.comparison_blocks(cells)
        self.assertEqual(len(blocks),10)
        self.assertEqual(len({b['id'] for b in blocks}),10)
        actual={(b['source_cell'],a,r) for b in blocks for a in c.arms(b)
                if not c.is_native(a) for r in c.repetition_positions(b)}
        expected={(s['id'],a,r) for s in cells for a in c.arms(s)
                  if not c.is_native(a) for r in range(1,s['repetitions']+1)}
        self.assertEqual(actual,expected)
        self.assertEqual(sum(a=='fereg' for b in blocks for a in c.arms(b)),10)
        self.assertTrue(all(b['repetitions']==1 and b['source_repetitions'] in (2,3) for b in blocks))
        budgets={(b['block_family'],b['block_repetition']):b['hard_budget_seconds'] for b in blocks}
        self.assertEqual(budgets['fixest',1],36_000)
        self.assertEqual(budgets['areg',1],27_000)
        self.assertEqual(budgets['fixest',2],18_000)
        bundles,bounds=c.pack_comparison_blocks(blocks)
        self.assertTrue(all(x<=37_800 for x in bounds))
        self.assertEqual(sum(bounds),sum(b['hard_budget_seconds'] for b in blocks))
        self.assertTrue(all(c.validate_block_bundle(b) for b in bundles))
        self.assertEqual(c.pack_comparison_blocks(list(reversed(blocks))), (bundles,bounds))

    def test_rejects_incomplete_pairs_stale_bounds_and_overbudget(self):
        with self.assertRaisesRegex(ValueError,'default/strict'):
            c.comparison_blocks([source(arms=['fereg','fixest'])])
        blocks=c.comparison_blocks([source()])
        with self.assertRaisesRegex(ValueError,'budget changed'):
            c.validate_block_bundle([dict(blocks[0],hard_budget_seconds=1)])
        with self.assertRaisesRegex(ValueError,'exceeds allocation'):
            c.validate_block_bundle([blocks[0],blocks[1]])
        with patch.dict(c.POLICY,block_bundle_seconds=43_200):
            with self.assertRaisesRegex(ValueError,'headroom'):
                c.pack_comparison_blocks(blocks)

    def test_prepare_includes_audited_panel_and_keeps_source_identity(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/'input').mkdir()
            panel=source(id='veneto-original',section='veneto',design='veneto',rows=20_000)
            smoke=source(id='veneto-smoke',section='veneto-smoke',design='veneto',repetitions=1)
            c.write_json(root/'input/additional-cells.json',[panel])
            c.write_json(root/'input/additional-smoke-cells.json',[smoke])
            with patch.object(c,'configurations',return_value=[source()]):c.prepare(root)
            manifest=json.loads((root/'manifest.json').read_text())
            self.assertEqual([s['id'] for s in manifest['source_cells']],['ols-panel','veneto-original'])
            self.assertEqual(manifest['additional_smoke_cells'],[smoke])
            self.assertEqual(manifest['policy']['bundles'],len(manifest['bundles']))
            self.assertEqual(manifest['timed_fits'],len(manifest['cells'])*4)
            self.assertTrue(all(x['section']!='reference' for x in manifest['cells']))
            self.assertEqual(c.controls_suffix(smoke),'')
            c.write_json(root/'input/additional-smoke-cells.json',[dict(smoke,rows=1201)])
            with self.assertRaisesRegex(ValueError,'dense oracle'):c.additional_cells(root,True)

    def test_timeout_does_not_skip_later_frozen_repetitions_or_larger_cell(self):
        cells=[source(arms=['fereg','fereg-strict','fixest','fixest-strict']),
               source(id='larger',rows=2400,repetitions=1,arms=['fereg','fereg-strict','fixest','fixest-strict'])]
        blocks=c.comparison_blocks(cells)
        # Saved cells avoid diagnostic refits; the mock never runs an estimator.
        blocks=[dict(b,save_fe=True,hard_budget_seconds=18_000) for b in blocks]
        calls=[]
        def fake_fit(root,cell,arm,repetition,frame,cpus):
            calls.append((cell['source_cell'],arm,repetition))
            return dict(cell=cell['id'],arm=arm,repetition=repetition,
                        status='command-timeout' if arm=='fixest' and repetition==1 else 'ok',metadata={})
        def make(cell,path):
            path.write_bytes(b'fixture')
            return pd.DataFrame({'x':[1.]}),{'sha256':c.sha256(path)}
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/'source').mkdir();(root/'environment').mkdir()
            # A timeout must not suppress either a larger cell in the same
            # allocation or the remaining predetermined source repetitions.
            bundles=[[blocks[0],blocks[3]],[blocks[1],blocks[2]]]
            c.write_json(root/'manifest.json',{'bundles':bundles})
            with patch.dict(c.POLICY,use_node_scratch=False,skip_larger_after_timeout=True),patch.object(c,'verify_runtime'), \
                 patch.object(c.pd,'read_stata',return_value=pd.DataFrame({'x':[1.]})), \
                 patch.object(c,'physical_cpus',return_value=[0]),patch.object(c,'make_dataset',side_effect=make), \
                 patch.object(c,'fit',side_effect=fake_fit),patch.object(c,'compare_records',return_value={'agreement':'differs'}):
                for task in range(1,len(bundles)+1):c.run_bundle(root,task)
            for task in range(1,len(bundles)+1):
                result=json.loads((root/f'task-{task}/results.json').read_text())
                self.assertTrue(result['complete'])
                self.assertEqual(result['records'][0]['source_cell'],bundles[task-1][0]['source_cell'])
                self.assertTrue((root/f'task-{task}/COMPLETE').exists())
            self.assertIn(('ols-panel','fixest',2),calls)
            self.assertIn(('ols-panel','fixest',3),calls)
            self.assertIn(('larger','fixest',1),calls)
            self.assertEqual(len(calls),16)

    def test_compact_retention_hashes_rows_and_preserves_comparisons(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);work=root/'scratch';task=root/'task';task.mkdir()
            (work/'data').mkdir(parents=True);(work/'data/meta.json').write_text('{}')
            (work/'data/input.dta').write_bytes(b'private input')
            records=[]
            for label in ['main','diagnostic']:
                out=work/'output'/label;out.mkdir(parents=True)
                for name in ['rows.csv','sample.npz','coefficients.csv','covariance.csv','config.tsv','fit.log']:
                    (out/name).write_text(label+' '+name)
                records.append(dict(directory=str(out),numerical_capture={'beta':[1.]}))
            records[0]['prediction_diagnostic']=records[1]
            result={'records':[records[0]],'numerical_comparisons':[{'fitted_max_absolute':0.}]}
            rawhash=c.sha256(work/'output/main/rows.csv')
            c.compact_scratch_result(work,task,result)
            self.assertFalse(list(task.rglob('*.npz')))
            self.assertFalse(list(task.rglob('*.dta')))
            self.assertFalse(list(task.rglob('rows.csv')))
            self.assertTrue((task/'output/main/covariance.csv').exists())
            self.assertTrue((task/'data/meta.json').exists())
            self.assertEqual(records[0]['ephemeral_output_files']['rows.csv']['sha256'],rawhash)
            self.assertEqual(records[1]['directory'],str(task/'output/diagnostic'))
            self.assertEqual(result['retention']['excluded_files'],4)
            self.assertFalse(result['retention']['row_arrays_available_after_job'])
            self.assertEqual(result['numerical_comparisons'],[{'fitted_max_absolute':0.}])
            self.assertTrue((work/'output/main/rows.csv').exists())

    def test_smoke_uses_frozen_inventory_only_for_explicit_small_panel(self):
        synthetic=source(id='synthetic',section='smoke',repetitions=1,arms=['fereg'])
        panel=source(id='real-smoke',design='veneto',section='veneto-smoke',repetitions=1,arms=['fereg'])
        made=[];frozen=[]
        def make(cell,path):
            made.append(cell['id']);path.write_bytes(b'input')
            return pd.DataFrame({'x':[1.]}),{'sha256':c.sha256(path)}
        def freeze(cell,path,inventory):
            frozen.append(cell['id']);return make(cell,path)
        def fit(root,cell,arm,repetition,frame,cpus):
            return dict(cell=cell['id'],arm=arm,repetition=repetition,status='ok',memory_exact=True,metadata={})
        inventory={'datasets':{c.dataset_key(synthetic):{},c.dataset_key(panel):{}}}
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/'source').mkdir();(root/'environment').mkdir()
            c.write_json(root/'manifest.json',{'additional_smoke_cells':[panel]})
            with patch.dict(c.POLICY,use_node_scratch=False),patch.object(c,'verify_runtime'), \
                 patch.object(c,'configurations',return_value=[synthetic]),patch.object(c,'physical_cpus',return_value=[0]), \
                 patch.object(c,'make_dataset',side_effect=make),patch.object(c,'frozen_dataset',side_effect=freeze), \
                 patch.object(c,'fit',side_effect=fit),patch('campaign_oracle.validate_smoke',return_value=[{'arm':'fereg','pass':True}]):
                c.run_bundle(root,1,smoke=True,frozen_inputs=inventory)
            self.assertEqual(frozen,['real-smoke'])
            result=json.loads((root/'smoke/results.json').read_text())
            self.assertTrue(result['native_gate_pass'])
            self.assertEqual(len(result['cells']),2)

    def test_scratch_requires_valid_tmpdir_before_generating_inputs(self):
        block=c.comparison_blocks([source()])[0]
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);c.write_json(root/'manifest.json',{'bundles':[[block]]})
            with patch.object(c,'verify_runtime'),patch.object(c,'physical_cpus',return_value=[0]), \
                 patch.dict(os.environ,TMPDIR='relative/path'),patch.object(c,'make_dataset') as make:
                with self.assertRaisesRegex(ValueError,'writable absolute TMPDIR'):c.run_bundle(root,1)
                make.assert_not_called()


if __name__=='__main__':unittest.main()
