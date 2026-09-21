"""Primary and paired inventories must preserve incomplete native attempts."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / 'benchmarks/two_fe_dense'))
import audit_milestone


class GraphInventoryAuditTests(unittest.TestCase):
    def run_fixture(self, variants, counts, *, corrupt=None, failed=None, legacy=False):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            cell = dict(weight_kind='aw')
            plan = dict(cells=[cell])
            if legacy:
                plan.update(baseline='old', candidate='new', repeats=3)
            else:
                plan.update(variants=variants, repetitions=3)
            (root / 'screen-plan.json').write_text(json.dumps(plan))
            (root / 'screen-progress.json').write_text(json.dumps([
                dict(index=0, cell=cell, label='c000')]))
            for variant in variants:
                for rep in range(counts[variant]):
                    run = root / 'runs' / f'c000-r{rep}-{variant}'
                    run.mkdir(parents=True)
                    result = dict(status='fail' if failed == (variant, rep) else 'pass',
                                  cell=dict(vce='robust', outputs='none'), sample_sha256='sample')
                    (run / 'result.json').write_text(json.dumps(result))
                    (run / 'request.json').write_text(json.dumps(dict(data='input.dta')))
                    for name in audit_milestone.EXPORTS:
                        (run / name).write_text('changed' if corrupt == (variant, rep) else 'same')
            # This fixture tests the controller's completeness decisions, not
            # the independent numerical oracle already exercised by the fits.
            def compare(path, oracle):
                return dict(status='mismatch' if path.parent.name.endswith('fixest') else 'pass')
            output = root / 'audit.json'
            with mock.patch.object(audit_milestone.graph_oracle, 'solve', return_value={}), \
                 mock.patch.object(audit_milestone.graph_oracle, 'compare', side_effect=compare), \
                 contextlib.redirect_stdout(io.StringIO()):
                audit_milestone.audit(root, 'screen', output)
            return json.loads(output.read_text())

    def test_primary_and_legacy_pairs_keep_reference_disagreements_separate(self):
        for variants, legacy in [(['new', 'fixest'], False), (['old', 'new', 'fixest'], True)]:
            with self.subTest(variants=variants):
                result = self.run_fixture(variants, {v: 3 for v in variants}, legacy=legacy)
                self.assertEqual(result['native_complete'], 1)
                self.assertEqual(result['records'][0]['attempts'][-1]['status'], 'mismatch')
                self.assertEqual(result['repetitions'], 3)

    def test_missing_extra_failed_and_changed_native_runs_cannot_pass(self):
        scenarios = [({'old': 2, 'new': 4}, None, None),
                     ({'old': 3, 'new': 2}, None, None),
                     ({'old': 3, 'new': 4}, None, None),
                     ({'old': 3, 'new': 3}, ('new', 2), None),
                     ({'old': 3, 'new': 3}, None, ('old', 1))]
        for counts, corrupt, failed in scenarios:
            with self.subTest(counts=counts, corrupt=corrupt, failed=failed):
                result = self.run_fixture(['old', 'new'], counts, corrupt=corrupt, failed=failed)
                self.assertTrue(result['complete'])
                self.assertEqual(result['native_complete'], 0)
                self.assertEqual(result['records'][0]['status'], 'native_incomplete_or_mismatch')


if __name__ == '__main__':
    unittest.main()
