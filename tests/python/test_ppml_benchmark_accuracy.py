"""Explicit comparator accuracy cannot silently weaken strict PPML screens."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'benchmarks/next_subsystem'))
from ppml_surface import configure


class ComparatorAccuracyTests(unittest.TestCase):
    def cell(self, **options):
        return dict(model='ppml', k=2, effects=['g1','g2'], fixest_strict=True, **options)

    def test_old_default_and_explicit_stronger_request_are_distinct_receipts(self):
        self.assertNotIn('fixest_glm_tol', configure(self.cell()))
        self.assertEqual(configure(self.cell(fixest_glm_tol=1e-12))['fixest_glm_tol'],1e-12)

    def test_weak_nonfinite_and_untyped_requests_fail_closed(self):
        for tolerance in [1e-8, 0, -1, True, float('nan'), float('inf'), '1e-12', None]:
            with self.subTest(tolerance=tolerance), self.assertRaises(ValueError):
                configure(self.cell(fixest_glm_tol=tolerance))
        cell = self.cell(fixest_glm_tol=1e-12); cell['fixest_strict'] = False
        with self.assertRaises(ValueError):
            configure(cell)


if __name__ == '__main__':
    unittest.main()
