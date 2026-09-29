import csv
import json
import importlib.util
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('audit', Path(__file__).resolve().parents[2]/'scripts/audit_pragmatic_memory.py')
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)

class MemoryAuditTests(unittest.TestCase):
    def fixture(self, root, predicted, actual):
        values = [1, 1, None, 1000, 0, predicted, 30, None, None, 512, 1]
        with (root/'fereg_memory.csv').open('w') as stream:
            w = csv.writer(stream); w.writerow(['row', 'column', 'value'])
            for row, value in enumerate(values, 1):
                w.writerow([row, 1, int(value is not None)])
                w.writerow([row, 2, value if value is not None else 0])
        (root/'stderr.txt').write_text('FEREG_WHOLE_ACTION_MEMORY_TERMINAL|version=1|estimator=ols|return_code=0|observation=abi_return|start_bytes=0|current_bytes=12|peak_bytes='+str(actual)+'|peak_delta_bytes='+str(actual)+'\n')

    def test_two_sided_accuracy_and_partial_coverage_are_independent(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for predicted, passed in [(95, True), (105, True), (89, False), (111, False)]:
                self.fixture(root, predicted, 100)
                r = audit.audit_case(root, absolute=0)
                self.assertEqual(r['native_accuracy_pass'], passed)
                self.assertFalse(r['whole_action_accepted'])
            self.fixture(root, None, 100)
            self.assertFalse(audit.audit_case(root)['native_accuracy_pass'])

    def test_inactive_stata_e_matrix_stub_is_not_the_native_report(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.fixture(root, 100, 100)
            (root/'feppml_memory.csv').write_text('row,column,value\n1,1,.\n')
            (root/'result.json').write_text(json.dumps({'arm': 'fereg'}))
            self.assertTrue(audit.audit_case(root)['native_accuracy_pass'])

    def test_practical_schema_preserves_proof_gaps_and_checks_phase_composition(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            self.fixture(root, 100, 100)
            def report(whole=1140, reserve=100, gaps=125):
                values = [2, 2, 800, 1000, 40, 100, 30, whole, reserve, 512, gaps]
                with (root/'fereg_memory.csv').open('w') as stream:
                    w = csv.writer(stream); w.writerow(['row','column','value'])
                    for row, value in enumerate(values, 1):
                        w.writerow([row,1,1]); w.writerow([row,2,value])
            report()
            self.assertEqual(audit.audit_case(root)['model_gap_bits'], 125)
            for args in [(1139,100,125), (1140,130,125), (1140,100,2**17)]:
                report(*args)
                with self.assertRaises(ValueError): audit.audit_case(root)

    def test_published_wire_versions_match_package_metadata(self):
        root = Path(__file__).resolve().parents[2]
        package = (root/'scripts/stage_release.py').read_text()
        self.assertIn('"memory_result_schema": 2', package)
        for filename, prefix, fields in [
            ('_fereg_estimate_ols.ado', 'fereg', {'action_protocol': '1.6', 'result_wire': '1.5'}),
            ('_fereg_estimate_ppml.ado', 'feppml', {'action_protocol': '1.6', 'request_wire': '1.8', 'result_wire': '1.5'}),
        ]:
            source = (root/'stata'/filename).read_text()
            for key, version in fields.items():
                self.assertIn(f'ereturn local {prefix}_{key} "{version}"', source)
                manifest_key = key if key == 'action_protocol' else ('ols_' if prefix == 'fereg' else 'ppml_') + key
                self.assertIn(f'"{manifest_key}": "{version}"', package)

    def test_missing_or_duplicate_terminal_observation_fails_closed(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); self.fixture(root, 100, 100)
            p = root/'stderr.txt'; p.write_text(p.read_text()*2)
            with self.assertRaises(ValueError): audit.audit_case(root)
            p.write_text('')
            with self.assertRaises(ValueError): audit.audit_case(root)

if __name__ == '__main__': unittest.main()
