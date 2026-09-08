import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('acceptance', root/'scripts/ppml_auto_acceptance.py')
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)


class AcceptanceTests(unittest.TestCase):
    def test_scope_inventory(self):
        manifest, _ = acceptance.manifest_scope(1)
        previous = json.loads((root/'benchmarks/manifests/feppml_auto_science_admission_v2.json').read_text())
        for key in ('datasets', 'expected_scopes', 'seconds', 'scheduler', 'reference_policy'):
            self.assertEqual(manifest[key], previous[key])
        self.assertNotIn('strict_inner_tolerance_for_both', manifest)
        self.assertEqual(manifest['checkpoint_projection_policy'], {
            'initial_strict_target': 1e-12, 'profile_rejection_refinement_factor': .01,
            'refinement_floor': 1e-14, 'preserve_already_tighter_target': True,
            'refined_target_applies_to_final_hessian': True, 'publication_tolerances_unchanged': True})
        self.assertEqual(acceptance.manifest_scope(1)[1]['threads'], 1)
        self.assertEqual(acceptance.manifest_scope(17)[1]['seed'], 2026091421)
        self.assertEqual(acceptance.manifest_scope(21)[1]['threads'], 28)
        for task in (0, 22, True):
            with self.assertRaises(ValueError):
                acceptance.manifest_scope(task)

    def test_source_identity_changes_and_escaping_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root/'source'
            source.write_text('first')
            identity = {'source': acceptance.digest(source)}
            acceptance.verify_source(root, identity)
            source.write_text('second')
            with self.assertRaises(ValueError):
                acceptance.verify_source(root, identity)
            with self.assertRaises(ValueError):
                acceptance.verify_source(root, {'../source': 'x'})
            with self.assertRaises(ValueError):
                acceptance.verify_source(root, {})

    def test_process_success_requires_sentinel(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)/'good.console'
            result = acceptance.run_process([sys.executable, '-c', 'print("PASS")'],
                dict(os.environ), output, 10, 'PASS')
            self.assertTrue(result['accepted'])
            bad = Path(directory)/'bad.console'
            with self.assertRaises(ValueError):
                acceptance.run_process([sys.executable, '-c', 'pass'], dict(os.environ), bad, 10, 'PASS')
            self.assertFalse(json.loads(bad.with_suffix('.process.json').read_text())['accepted'])

    def test_timeout_cannot_accept_printed_success(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)/'timeout.console'
            with self.assertRaises(ValueError):
                acceptance.run_process([sys.executable, '-u', '-c', 'import time; print("PASS"); time.sleep(10)'],
                    dict(os.environ), output, .2, 'PASS')
            result = json.loads(output.with_suffix('.process.json').read_text())
            self.assertTrue(result['timed_out'])
            self.assertFalse(result['accepted'])


if __name__ == '__main__':
    unittest.main()
