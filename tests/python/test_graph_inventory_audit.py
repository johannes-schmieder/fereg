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
    def run_fixture(self, variants, counts, *, corrupt=None, failed=None, legacy=False, cache_mode=None):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            cell = dict(weight_kind='aw')
            data = root / 'input.dta'
            data.write_text('fixture input')
            input_sha = audit_milestone.sha(data)
            oracle = dict(vce='robust', beta=[1.0])
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
                                  cell=dict(vce='robust', outputs='none'), sample_sha256='sample', input_sha256=input_sha)
                    (run / 'result.json').write_text(json.dumps(result))
                    (run / 'request.json').write_text(json.dumps(dict(data=str(data))))
                    for name in audit_milestone.EXPORTS:
                        (run / name).write_text('changed' if corrupt == (variant, rep) else 'same')
            # This fixture tests the controller's completeness decisions, not
            # the independent numerical oracle already exercised by the fits.
            def compare(path, oracle):
                return dict(status='mismatch' if path.parent.name.endswith('fixest') else 'pass')
            cache_path = None
            if cache_mode:
                cached = dict(complete=True, planned=1, native_complete=1,
                    oracle_source_sha256=audit_milestone.sha(Path(audit_milestone.graph_oracle.__file__)),
                    records=[dict(status='native_pass', cell=cell, attempts=[dict(variant='prior', status='pass',
                        input_sha256='0' * 64 if cache_mode == 'miss' else input_sha, oracle=oracle)])])
                if cache_mode == 'wrong_source':
                    cached['oracle_source_sha256'] = '0' * 64
                elif cache_mode == 'incomplete':
                    cached['complete'] = False
                elif cache_mode == 'missing_record':
                    cached['records'] = []
                elif cache_mode == 'conflict':
                    cached['records'][0]['attempts'].append(dict(variant='second', status='pass',
                        input_sha256=input_sha, oracle=dict(vce='robust', beta=[2.0])))
                elif cache_mode == 'changed_input':
                    data.write_text('changed input')
                cache_path = root / 'prior-audit.json'
                cache_path.write_text(json.dumps(cached))
            output = root / 'audit.json'
            with mock.patch.object(audit_milestone.graph_oracle, 'solve', return_value=oracle) as solve, \
                 mock.patch.object(audit_milestone.graph_oracle, 'compare', side_effect=compare), \
                 contextlib.redirect_stdout(io.StringIO()):
                audit_milestone.audit(root, 'screen', output, cache_path)
            result = json.loads(output.read_text())
            result['_fresh_solve_calls'] = solve.call_count
            return result

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

    def test_cache_reuses_only_identical_inputs_and_solves_new_inputs(self):
        for mode, calls, origin in [('hit', 0, 'verified_cache'), ('miss', 1, 'fresh_solve')]:
            with self.subTest(mode=mode):
                result = self.run_fixture(['new'], {'new': 3}, cache_mode=mode)
                self.assertEqual(result['native_complete'], 1)
                self.assertEqual(result['_fresh_solve_calls'], calls)
                self.assertEqual(result['records'][0]['attempts'][0]['oracle_origin'], origin)
                self.assertIsNotNone(result['oracle_cache_sha256'])

    def test_invalid_cache_or_changed_actual_input_cannot_pass(self):
        for mode in ['wrong_source', 'incomplete', 'missing_record', 'conflict', 'changed_input']:
            with self.subTest(mode=mode), self.assertRaises(AssertionError):
                self.run_fixture(['new'], {'new': 3}, cache_mode=mode)


if __name__ == '__main__':
    unittest.main()
