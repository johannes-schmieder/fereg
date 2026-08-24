from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OLS = ROOT / "crates/fereg-core/src/ols"


class OlsSourceContractTests(unittest.TestCase):
    def test_rank_solver_contains_both_certified_paths(self):
        text = (OLS / "rank.rs").read_text(encoding="utf-8")
        self.assertIn("PivotedCholesky", text)
        self.assertIn("ColumnPivotedQr", text)
        self.assertIn("solve_pivoted_cholesky", text)
        self.assertIn("solve_column_pivoted_qr", text)
        self.assertNotIn("solve_linear_system", text)

    def test_fwl_requires_certified_absorption(self):
        text = (OLS / "mod.rs").read_text(encoding="utf-8")
        self.assertIn("absorb_fixed_effects", text)
        self.assertIn("if !report.certified()", text)
        self.assertIn("CollinearWithFixedEffects", text)

    def test_cluster_vce_uses_tuple_intersections_and_inclusion_exclusion(self):
        text = (OLS / "vce.rs").read_text(encoding="utf-8")
        self.assertIn("interaction_codes", text)
        self.assertIn("1_usize << dimensions", text)
        self.assertIn("subset.len() % 2", text)
        self.assertNotIn('format!("{}#{}"', text)

    def test_approximate_dof_is_explicit(self):
        text = (OLS / "dof.rs").read_text(encoding="utf-8")
        self.assertIn("absorbed_rank_is_approximate", text)
        self.assertIn("DofMethod::Conservative", text)


if __name__ == "__main__":
    unittest.main()
