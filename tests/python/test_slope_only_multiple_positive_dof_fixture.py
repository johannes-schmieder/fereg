from __future__ import annotations

import csv
import hashlib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests/fixtures/ols/ols_slope_only_multiple_positive_dof.csv"


class SlopeOnlyMultiplePositiveDofFixtureTests(unittest.TestCase):
    def test_fixture_matches_the_independent_oracle_formula(self) -> None:
        with FIXTURE.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
        self.assertEqual(len(rows), 60)
        for index, row in enumerate(rows):
            firm = index % 5
            worker = (index // 5) % 6
            time = index % 7 - 3
            x2 = ((index * 11 + 1) % 23) / 4 - 2.5
            x1 = ((index * 5 + 2) % 19) / 3 - 3
            noise = ((index * 13 + 4) % 17) / 100 - 0.08
            expected_y = (
                1.5
                + 0.7 * x1
                + (0.08 * (firm + 1) - 0.05 * (worker + 1)) * time
                + 0.03 * (worker + 2) * x2
                + noise
            )
            self.assertEqual(int(row["obs_id"]), index + 1)
            self.assertEqual(int(row["firm"]), firm + 1)
            self.assertEqual(int(row["worker"]), worker + 1)
            self.assertAlmostEqual(float(row["time"]), time, places=12)
            self.assertAlmostEqual(float(row["x1"]), x1, places=11)
            self.assertAlmostEqual(float(row["x2"]), x2, places=12)
            self.assertAlmostEqual(float(row["y"]), expected_y, places=11)

    def test_fixture_has_positive_conservative_residual_dof(self) -> None:
        with FIXTURE.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
        firm_categories = {row["firm"] for row in rows}
        worker_categories = {row["worker"] for row in rows}
        conservative_absorbed_rank = len(firm_categories) + 2 * len(worker_categories)
        self.assertEqual(conservative_absorbed_rank, 17)
        # Pinned reghdfe reports no structural constant for an all-slope-only
        # absorb list, so only the identified x1 direction is finite-model DoF.
        self.assertEqual(len(rows) - conservative_absorbed_rank - 1, 42)

    def test_fixture_bytes_are_deliberately_bound(self) -> None:
        self.assertEqual(
            hashlib.sha256(FIXTURE.read_bytes()).hexdigest(),
            "303427127f99114d274d3c4b9888a1060be41744dc2ced80108bcff99cbf0808",
        )


if __name__ == "__main__":
    unittest.main()
