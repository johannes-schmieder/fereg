import copy
import csv
import tempfile
import unittest
from pathlib import Path

from campaign import build_plan
from common import HERE, cell_id, read_json, write_json
from fixest_sweep import register
from light import active_arms, cells, groups, skips


class FixestSweepTests(unittest.TestCase):
    def setUp(self):
        self.original = read_json(HERE / 'manifest-light-v2.json')
        self.history = HERE.parent / 'reports/development/systematic-light-scc-final-2026-09-27/generated/runs.csv'
        self.manifest = register(self.original, self.history, 'test-candidate')

    def test_preserves_all_scientific_inputs_and_native_coverage(self):
        self.assertEqual(cells(self.manifest), cells(self.original))
        self.assertEqual(groups(self.manifest), groups(self.original))
        self.assertEqual(len(cells(self.manifest)), 140)
        self.assertEqual(len(groups(self.manifest)), 568)
        for key in ('science', 'settings', 'seeds', 'fixest_strict', 'measurement'):
            self.assertEqual(self.manifest[key], self.original[key])
        for group in groups(self.manifest):
            self.assertIn(self.manifest['arms'][group['cell']['model']][0], active_arms(group, self.manifest))
        self.assertEqual(self.original, read_json(HERE / 'manifest-light-v2.json'))

    def test_only_exact_failed_reference_seeds_are_excluded(self):
        with self.history.open() as stream:
            rows = list(csv.DictReader(stream))
        expected = {(cell_id(r), int(r['repetition']), r['arm']) for r in rows
                    if r['arm'] in ('fixest', 'fixest_strict') and r['status'] == 'command_timeout'
                    and not r['section'].startswith('slopes_')}
        observed = {(cell_id(g['cell']), g['repetition'], arm) for g in groups(self.manifest)
                    for arm in skips(g['cell'], g['repetition'], self.manifest)}
        self.assertEqual(observed, expected)
        self.assertEqual(len(observed), 50)
        # Successful third seed stays live even when its neighbors timed out.
        group = next(g for g in groups(self.manifest) if g['cell']['model'] == 'ppml'
                     and g['cell']['design'] == 'two_fe_sparse' and g['cell']['n'] == 3932160
                     and g['cell']['threads'] == 4 and g['repetition'] == 3)
        self.assertIn('fixest_strict', active_arms(group, self.manifest))

    def test_rejects_missing_and_duplicate_history(self):
        with self.history.open() as stream:
            reader = csv.DictReader(stream)
            fields, rows = reader.fieldnames, list(reader)
        row = next(r for r in rows if r['arm'] == 'fereg')
        for bad in ([r for r in rows if r is not row], [*rows, row]):
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'history.csv'
                with path.open('w') as stream:
                    writer = csv.DictWriter(stream, fieldnames=fields)
                    writer.writeheader()
                    writer.writerows(bad)
                with self.assertRaises(ValueError):
                    register(self.original, path, 'candidate')

    def test_pilot_uses_registered_selection_and_rejects_invalid_ids(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'input').mkdir()
            (root / 'input/historical-runs.csv').write_bytes(self.history.read_bytes())
            write_json(root / 'input/manifest.json', self.manifest)
            plan = build_plan(self.manifest, 'pilot', root / 'results')
            selected = [g for b in plan['bundles'] for g in b['groups']]
            self.assertEqual(len(selected), 8)
            self.assertEqual({cell_id(g['cell']) for g in selected}, set(self.manifest['pilot_cell_ids']))
            self.assertTrue(all(g['repetition'] == 1 for g in selected))
            for ids in ([], ['missing'], self.manifest['pilot_cell_ids'] * 2):
                invalid = copy.deepcopy(self.manifest)
                invalid['pilot_cell_ids'] = ids
                with self.assertRaises(ValueError):
                    build_plan(invalid, 'pilot', root / 'results')


if __name__ == '__main__':
    unittest.main()
