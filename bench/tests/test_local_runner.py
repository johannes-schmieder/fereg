"""The small benchmark must retain failed cells and immutable input meaning."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from generate import generate
from run import load_suite, selected_cells, write_configuration, launch
from compare import summarize, stata_hex_to_float


class LocalBenchmarkTests(unittest.TestCase):
    def test_inputs_are_byte_reproducible(self):
        cell = dict(selected_cells(load_suite(), 'smoke')[0], rows=100)
        with tempfile.TemporaryDirectory() as directory:
            a = generate(cell, 123, Path(directory)/'a.dta')
            b = generate(cell, 123, Path(directory)/'b.dta')
            self.assertEqual(a['sha256'], b['sha256'])

    def test_missing_workload_or_duplicate_repetitions_cannot_pass(self):
        suite = load_suite()
        run = dict(suite=suite, tier='smoke', complete=True, records=[])
        self.assertFalse(summarize(run)['accuracy_pass'])
        records=[]
        for cell in suite['cells']:
            records.append(dict(cell=cell['id'], threads=1, reference=True, status='ok'))
            for i in range(6):
                records.append(dict(cell=cell['id'], threads=1, repetition=i,
                                    reference=False, status='ok', metadata={'seconds':'0.2'}))
        run['records']=records
        with patch('compare.accuracy',return_value=('pass',0)), patch('compare.scientific_accuracy',return_value=True):
            self.assertTrue(summarize(run)['accuracy_pass'])
            records[-1]['repetition']=1
            self.assertFalse(summarize(run)['accuracy_pass'])

    def test_configuration_cannot_inject_stata_code(self):
        with tempfile.TemporaryDirectory() as directory:
            for value in ['bad"path', 'bad\npath', '$macro', '`macro']:
                with self.assertRaises(ValueError):
                    write_configuration(Path(directory), dict(source=value))

    def test_process_errors_and_timeouts_stay_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            result=launch([sys.executable,'-c','raise SystemExit(3)'],Path(directory),10)
            self.assertEqual(result['process_status'],'error')
            result=launch([sys.executable,'-c','import time;time.sleep(3)'],Path(directory),.05)
            self.assertEqual(result['process_status'],'timeout')

    def test_canonical_stata_hex(self):
        self.assertEqual(stata_hex_to_float('+1.8000000000000X+001'),3)
        with self.assertRaises(ValueError):stata_hex_to_float('unparseable')


if __name__ == '__main__':unittest.main()
