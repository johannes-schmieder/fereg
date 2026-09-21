"""Partial baseline failures must not erase valid comparator evidence."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'benchmarks/two_fe_dense'))
import score_milestone


class MilestoneScoreTests(unittest.TestCase):
    def fixture(self, root, failed=None, weight='none', isolation='no_detected_overlap'):
        cell = dict(design='cycle', n=1920, k=6, threads=16, seed=8675309,
            shuffled=False, weight_kind=weight, weight_design='none' if weight == 'none' else 'moderate',
            case='robust_none', effects=['g1', 'g2'], solver='auto', memory='fast')
        plan = dict(schema='ols-graph-coverage-execution-v1', phase='screen',
            baseline='baseline', candidate='candidate', repeats=3, cells=[cell])
        attempt = dict(index=0, cell=cell, label='screen-c0000', process_returncode=1 if failed else 0,
            comparison_status='recorded_application_failure' if failed else 'pass')
        if isolation is not None:
            attempt['timing_isolation'] = dict(status=isolation)
        (root / 'screen-plan.json').write_text(json.dumps(plan))
        (root / 'screen-progress.json').write_text(json.dumps([attempt]))
        variants = ['baseline', 'candidate'] + ([] if weight == 'fw' else ['fixest'])
        result_cell = dict(cell, fit_options='memory(fast)', fixest_tolerance=1e-6,
            fixest_recovery_tolerance=1e-5)
        for rep in range(1, 4):
            for variant in variants:
                run = root / 'runs' / f'screen-c0000-test-r{rep}-{variant}'
                run.mkdir(parents=True)
                fingerprint = dict(fixest_archive_sha256='reference-archive', fixest_library='pinned-library') \
                    if variant == 'fixest' else dict(native=dict(abi='pass', plugin=variant))
                result = dict(status='fail' if failed == variant else 'pass',
                    process_returncode=459 if failed == variant else 0, sentinel=failed != variant,
                    seconds=dict(baseline=2.0, candidate=1.0, fixest=3.0)[variant], cell=result_cell,
                    measurement_fingerprint=fingerprint, input_sha256='input', sample_sha256='sample', sample_rows=1920)
                (run / 'result.json').write_text(json.dumps(result))
                (run / 'request.json').write_text(json.dumps(dict(solver='auto')))
                (run / 'fereg_runtime.csv').write_text('row,column,value\n13,2,1024\n')

    def test_failed_baseline_remains_failed_but_reference_pairs_survive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, failed='baseline')
            score = score_milestone.score(root, 'screen')
            cell = score['cells'][0]
            self.assertEqual(score['completely_measured'], 0)
            self.assertFalse(score['identity_audit_complete'])
            self.assertEqual(score['comparable_outcomes'], {'win': 1})
            self.assertEqual(len(cell['comparator_pairs']), 3)
            self.assertEqual(cell['median_comparator_ratio'], 1 / 3)
            self.assertNotIn('median_native_ratio', cell)
            self.assertIn('rep1:baseline:application_failure', cell['issues'])

    def test_failed_candidate_or_reference_cannot_rank(self):
        for variant in ['candidate', 'fixest']:
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.fixture(root, failed=variant)
                score = score_milestone.score(root, 'screen')
                self.assertEqual(score['comparator_complete'], 0)
                self.assertEqual(score['comparable_outcomes'], {})

    def test_overlap_and_unrecorded_isolation_keep_times_but_cannot_rank(self):
        for isolation in ['possible_overlap', None]:
            with self.subTest(isolation=isolation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.fixture(root, isolation=isolation)
                score = score_milestone.score(root, 'screen')
                self.assertEqual(score['comparator_complete'], 1)
                self.assertEqual(score['comparable_timing_eligible'], 0)
                self.assertEqual(score['comparable_outcomes'], {})

    def test_fw_is_a_native_control(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, weight='fw')
            score = score_milestone.score(root, 'screen')
            self.assertEqual(score['fw_native_controls'], 1)
            self.assertEqual(score['cells'][0]['median_native_ratio'], 0.5)
            self.assertEqual(score['comparable_outcomes'], {})

    def test_partial_pair_still_requires_identical_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, failed='baseline')
            path = next((root / 'runs').glob('*r1-fixest/result.json'))
            result = json.loads(path.read_text())
            result['input_sha256'] = 'different-input'
            path.write_text(json.dumps(result))
            with self.assertRaises(AssertionError):
                score_milestone.score(root, 'screen')

    def test_complete_triplets_keep_original_native_and_reference_ratios(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            score = score_milestone.score(root, 'screen')
            self.assertEqual(score['completely_measured'], 1)
            self.assertTrue(score['identity_audit_complete'])
            self.assertEqual(score['cells'][0]['median_native_ratio'], 0.5)
            self.assertEqual(score['cells'][0]['median_paired_ratio'], 1 / 3)


if __name__ == '__main__':
    unittest.main()
