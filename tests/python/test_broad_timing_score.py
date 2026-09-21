"""Broad protection blocks may rank only recorded, isolated measurements."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'benchmarks/two_fe_dense'))
import score


class BroadTimingScoreTests(unittest.TestCase):
    def fixture(self, root, isolation, controls=False, failed=False):
        cell = dict(design='two_fe_dense', n=1920, k=6, threads=16, seed=8675309,
            shuffled=False, weight_kind='none', weight_design='none', case='robust',
            effects=['g1', 'g2'], solver='auto', memory='fast')
        variants = ['baseline', 'candidate'] if controls else ['candidate', 'fixest']
        plan = dict(phase='controls' if controls else 'block-a', repetitions=1,
            inventory_kind='controls' if controls else 'primary', variants=variants, cells=[cell],
            manifest=dict(acceptance=dict(native_control_max_confirmed_regression=1.05,
                primary_max_median_paired_ratio=1.0)))
        attempt = dict(index=0, cell=cell, label='block-c000', process_returncode=int(failed),
            comparison_status='pass')
        if isolation is not None:
            attempt['timing_isolation'] = dict(status=isolation)
        (root / 'block-plan.json').write_text(json.dumps(plan))
        (root / 'block-progress.json').write_text(json.dumps([attempt]))
        for variant in variants:
            run = root / 'runs' / f'block-c000-test-r1-{variant}'
            run.mkdir(parents=True)
            fingerprint = dict(fixest_archive_sha256='reference', fixest_library='pinned') \
                if variant == 'fixest' else dict(native=dict(abi='pass', plugin=variant))
            result = dict(status='fail' if failed and variant == 'candidate' else 'pass',
                process_returncode=0, sentinel=True, seconds=1.0 if variant == 'candidate' else 2.0,
                cell=dict(cell, fit_options='memory(fast)', fixest_tolerance=1e-6,
                    fixest_recovery_tolerance=1e-5), measurement_fingerprint=fingerprint,
                input_sha256='input', sample_sha256='sample', sample_rows=1920)
            (run / 'result.json').write_text(json.dumps(result))
            (run / 'request.json').write_text(json.dumps(dict(solver='auto')))
            (run / 'fereg_runtime.csv').write_text('row,column,value\n13,2,1024\n')

    def test_clean_primary_and_native_control_can_rank(self):
        for controls in [False, True]:
            with self.subTest(controls=controls), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.fixture(root, 'no_detected_overlap', controls)
                result = score.score(root, 'block')
                self.assertTrue(result['timing_threshold_met'])
                self.assertEqual(result['wins'], 1)
                self.assertEqual(result['worst_ratio'], 0.5)

    def test_overlap_or_unrecorded_isolation_preserves_evidence_but_cannot_rank(self):
        for isolation in ['possible_overlap', None]:
            with self.subTest(isolation=isolation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.fixture(root, isolation)
                result = score.score(root, 'block')
                self.assertTrue(result['identity_audit_complete'])
                self.assertEqual(result['completely_measured'], 1)
                self.assertEqual(result['cells'][0]['median_paired_ratio'], 0.5)
                self.assertEqual(result['timing_eligible'], 0)
                self.assertEqual(result['wins'], 0)
                self.assertIsNone(result['worst_ratio'])
                self.assertFalse(result['timing_threshold_met'])

    def test_failed_application_does_not_rank(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, 'no_detected_overlap', failed=True)
            result = score.score(root, 'block')
            self.assertEqual(result['wins'], 0)
            self.assertFalse(result['identity_audit_complete'])
            self.assertFalse(result['timing_threshold_met'])


if __name__ == '__main__':
    unittest.main()
