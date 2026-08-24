from __future__ import annotations

import csv
import hashlib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ORIGINAL = ROOT / "tests/fixtures/ols/ols_core.csv"
POSITIVE_DOF = ROOT / "tests/fixtures/ols/ols_intercept_slope_positive_dof.csv"


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


class InterceptSlopePositiveDofFixtureTests(unittest.TestCase):
    def test_fixture_is_an_exact_twofold_topology_replication(self) -> None:
        original = read_rows(ORIGINAL)
        expanded = read_rows(POSITIVE_DOF)
        self.assertEqual(len(original), 24)
        self.assertEqual(len(expanded), 48)

        for copy_index in range(2):
            for row_index, source in enumerate(original):
                candidate = dict(expanded[copy_index * len(original) + row_index])
                self.assertEqual(
                    int(candidate.pop("obs_id")),
                    int(source["obs_id"]) + copy_index * len(original),
                )
                expected = dict(source)
                expected.pop("obs_id")
                self.assertEqual(candidate, expected)

    def test_fixture_preserves_fe_support_but_makes_conservative_dof_positive(self) -> None:
        original = read_rows(ORIGINAL)
        expanded = read_rows(POSITIVE_DOF)
        for field in ("firm", "worker", "time"):
            self.assertEqual(
                {row[field] for row in expanded},
                {row[field] for row in original},
            )

        # The pinned intercept-plus-slope command reports conservative df_a=26.
        # The former 24-row fixture therefore has df_r=-2 and remains the
        # over-absorption failure oracle. The dedicated fixture has df_r=22.
        self.assertEqual(len(original) - 26, -2)
        self.assertEqual(len(expanded) - 26, 22)

    def test_fixture_bytes_are_deliberately_bound(self) -> None:
        self.assertEqual(
            hashlib.sha256(POSITIVE_DOF.read_bytes()).hexdigest(),
            "b6986ed24a6266fd3addeea42e2a834b420c1f590bcc31f84f8584cc821ae3f8",
        )
        self.assertEqual(
            hashlib.sha256(ORIGINAL.read_bytes()).hexdigest(),
            "cb7d7e7ae21843f7c1dc5f84ac4daffa3954c9bfc54a9a109a3e2f6b50433bca",
        )


if __name__ == "__main__":
    unittest.main()
