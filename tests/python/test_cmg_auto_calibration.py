import unittest

from scripts.cmg_auto_calibration import (
    ContractError,
    analyze_rows,
    load_manifest,
    selects_cmg,
)


def row(case_id, observations, high, second, nuisance, threads, solver, seconds):
    return {
        "case_id": case_id,
        "observations": observations,
        "high_levels": high,
        "second_levels": second,
        "nuisance_columns": nuisance,
        "absorbed_slopes": 0,
        "threads": threads,
        "solver": solver,
        "seconds": seconds,
    }


class CmgAutoCalibrationTests(unittest.TestCase):
    def test_manifest_and_route_boundaries_are_frozen(self) -> None:
        load_manifest()
        self.assertTrue(selects_cmg(row("yes", 800, 100, 10, 0, 1, "cmg", 1.0)))
        self.assertFalse(selects_cmg(row("short", 799, 100, 10, 0, 1, "cmg", 1.0)))
        self.assertFalse(
            selects_cmg(row("balanced", 800, 100, 11, 0, 1, "cmg", 1.0))
        )
        self.assertFalse(
            selects_cmg(row("nuisance", 800, 100, 10, 1, 1, "cmg", 1.0))
        )

    def test_analyzer_accepts_faster_selected_aggregate_and_keeps_rejections_visible(
        self,
    ) -> None:
        rows = [
            row("selected", 800, 100, 10, 0, 1, "cmg", 0.7),
            row("selected", 800, 100, 10, 0, 1, "lsmr", 1.0),
            row("short", 700, 100, 10, 0, 1, "cmg", 2.0),
            row("short", 700, 100, 10, 0, 1, "lsmr", 1.0),
        ]
        report = analyze_rows(rows)
        self.assertEqual(len(report["selected"]), 1)
        self.assertEqual(len(report["rejected"]), 1)

    def test_analyzer_fails_closed_on_a_selected_regression(self) -> None:
        rows = [
            row("selected", 800, 100, 10, 0, 8, "cmg", 1.06),
            row("selected", 800, 100, 10, 0, 8, "lsmr", 1.0),
        ]
        with self.assertRaisesRegex(ContractError, "five percent slower"):
            analyze_rows(rows)


if __name__ == "__main__":
    unittest.main()
