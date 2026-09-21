"""Protection auditing must retain failed pairs and reject scientific changes."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'benchmarks/two_fe_dense'))
import audit_protection


class NativeProtectionAuditTests(unittest.TestCase):
    def fixture(self, root, defect):
        cell = dict(n=2, k=1, effects=['g1'], outputs='all')
        plan = dict(inventory_kind='controls', repetitions=1,
                    variants=['baseline', 'candidate'], cells=[cell])
        (root / 'check-plan.json').write_text(json.dumps(plan))
        (root / 'check-progress.json').write_text(json.dumps([
            dict(index=0, cell=cell, label='check-c000')]))
        for variant in plan['variants']:
            if defect == 'missing' and variant == 'candidate':
                continue
            run = root / 'runs' / f'check-c000-fit-r1-{variant}'
            run.mkdir(parents=True)
            result = dict(status='pass', arm='fereg', sentinel=True,
                          process_returncode=0, cell=cell, input_sha256='input',
                          sample_sha256='sample', metadata=dict(N=2, rss=3.0))
            if variant == 'candidate' and defect == 'sample':
                result['sample_sha256'] = 'different'
            if variant == 'candidate' and defect == 'failed':
                result['status'] = 'failed'
            if variant == 'candidate' and defect == 'rss':
                result['metadata']['rss'] = 6.0
            (run / 'result.json').write_text(json.dumps(result))
            coefficient = 2 if variant == 'candidate' and defect == 'coefficient' else 1
            (run / 'coefficients.csv').write_text(f'name,value,se\nx1,{coefficient},0.5\n')
            for name in ['covariance.csv', 'b.csv', 'V.csv', 'fereg_fit.csv',
                         'fereg_dof.csv', 'fereg_status.csv']:
                value = 2 if variant == 'candidate' and defect == name else 1
                (run / name).write_text(f'row,column,value\n1,1,{value}\n')
            values = np.zeros(8, dtype='<f8')
            if variant == 'candidate' and defect == 'outputs':
                values[0] = 1
            values.tofile(run / 'outputs.bin')

    def test_identical_pair_passes_with_exact_export_record(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root, None)
            output = root / 'audit.json'
            with contextlib.redirect_stdout(io.StringIO()):
                audit_protection.audit(root, 'check', output)
            result = json.loads(output.read_text())
            self.assertEqual(result['passing'], 1)
            self.assertTrue(all(result['records'][0]['pairs'][0]['exact_exports'].values()))

    def test_corruption_and_incomplete_attempts_remain_failures(self):
        for defect in ['missing', 'failed', 'sample', 'rss', 'coefficient', 'covariance.csv',
                       'fereg_dof.csv', 'fereg_status.csv', 'outputs']:
            with self.subTest(defect=defect), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                self.fixture(root, defect)
                output = root / 'audit.json'
                with contextlib.redirect_stdout(io.StringIO()):
                    audit_protection.audit(root, 'check', output)
                result = json.loads(output.read_text())
                self.assertEqual(result['passing'], 0)
                self.assertEqual(len(result['records']), 1)
                self.assertNotEqual(result['records'][0]['pairs'][0]['status'], 'pass')


if __name__ == '__main__':
    unittest.main()
