from __future__ import annotations

import csv
import math
import struct
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests/fixtures/ols/ols_ts.csv"


def stata_import_float(value: str) -> float:
    """Mirror import delimited's inferred float storage for this fixture."""
    return struct.unpack(">f", struct.pack(">f", float(value)))[0]


def invert(matrix: list[list[float]]) -> list[list[float]]:
    size = len(matrix)
    augmented = [
        row[:] + [float(index == column) for column in range(size)]
        for index, row in enumerate(matrix)
    ]
    for column in range(size):
        pivot = max(range(column, size), key=lambda row: abs(augmented[row][column]))
        if abs(augmented[pivot][column]) <= 1e-14:
            raise AssertionError("oracle design is singular")
        augmented[column], augmented[pivot] = augmented[pivot], augmented[column]
        scale = augmented[column][column]
        augmented[column] = [value / scale for value in augmented[column]]
        for row in range(size):
            if row == column:
                continue
            factor = augmented[row][column]
            augmented[row] = [
                value - factor * pivot_value
                for value, pivot_value in zip(augmented[row], augmented[column])
            ]
    return [row[size:] for row in augmented]


def matvec(matrix: list[list[float]], vector: list[float]) -> list[float]:
    return [sum(left * right for left, right in zip(row, vector)) for row in matrix]


def explicit_ols(
    design: list[list[float]], outcome: list[float]
) -> tuple[list[float], list[list[float]], list[float]]:
    columns = len(design[0])
    cross = [
        [sum(row[left] * row[right] for row in design) for right in range(columns)]
        for left in range(columns)
    ]
    rhs = [sum(row[column] * value for row, value in zip(design, outcome)) for column in range(columns)]
    bread = invert(cross)
    coefficients = matvec(bread, rhs)
    residuals = [
        value - sum(coefficient * regressor for coefficient, regressor in zip(coefficients, row))
        for row, value in zip(design, outcome)
    ]
    df_r = len(outcome) - columns
    scale = sum(value * value for value in residuals) / df_r
    covariance = [[scale * value for value in row] for row in bread]
    return coefficients, covariance, residuals


def transformed_covariance(
    transform: list[list[float]], covariance: list[list[float]]
) -> list[list[float]]:
    return [
        [
            sum(
                transform[left][row]
                * covariance[row][column]
                * transform[right][column]
                for row in range(len(covariance))
                for column in range(len(covariance))
            )
            for right in range(len(transform))
        ]
        for left in range(len(transform))
    ]


class TimeSeriesExplicitDummyOracleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        with FIXTURE.open(newline="", encoding="utf-8") as handle:
            cls.rows = [
                {
                    "obs_id": int(row["obs_id"]),
                    "panel": int(row["panel"]),
                    "year": int(row["year"]),
                    "y": stata_import_float(row["y"]),
                    "x": stata_import_float(row["x"]),
                }
                for row in csv.DictReader(handle)
            ]
        cls.by_panel_year = {
            (row["panel"], row["year"]): row for row in cls.rows
        }

    def test_lag_with_panel_dummies_matches_pinned_reference(self) -> None:
        selected = [
            row
            for row in self.rows
            if (row["panel"], row["year"] - 1) in self.by_panel_year
        ]
        self.assertEqual(
            [row["obs_id"] for row in selected],
            [2, 3, 4, 6, 8, 9, 10, 12, 14, 15, 16, 18],
        )
        design = [
            [
                self.by_panel_year[(row["panel"], row["year"] - 1)]["x"],
                1.0,
                float(row["panel"] == 2),
                float(row["panel"] == 3),
            ]
            for row in selected
        ]
        coefficients, covariance, _ = explicit_ols(
            design, [row["y"] for row in selected]
        )
        transform = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 1.0 / 3.0, 1.0 / 3.0]]
        reported = matvec(transform, coefficients)
        reported_covariance = transformed_covariance(transform, covariance)
        expected = [float.fromhex("0x1.270f02a5101a0p+0"), float.fromhex("0x1.556267538f239p+1")]
        expected_covariance = [
            [float.fromhex("0x1.ba1098b4b5749p-6"), -float.fromhex("0x1.6905058614c09p-6")],
            [-float.fromhex("0x1.6905058614c09p-6"), float.fromhex("0x1.7a4d99cd796b5p-5")],
        ]
        for actual, target in zip(reported, expected):
            self.assertTrue(math.isclose(actual, target, rel_tol=1e-7, abs_tol=1e-10))
        for actual_row, target_row in zip(reported_covariance, expected_covariance):
            for actual, target in zip(actual_row, target_row):
                self.assertTrue(math.isclose(actual, target, rel_tol=1e-6, abs_tol=1e-10))

    def test_lead_and_difference_gap_sample_matches_pinned_reference(self) -> None:
        selected = [
            row
            for row in self.rows
            if (row["panel"], row["year"] - 1) in self.by_panel_year
            and (row["panel"], row["year"] + 1) in self.by_panel_year
        ]
        self.assertEqual([row["obs_id"] for row in selected], [2, 3, 8, 9, 14, 15])
        design = [
            [
                self.by_panel_year[(row["panel"], row["year"] + 1)]["x"],
                row["x"] - self.by_panel_year[(row["panel"], row["year"] - 1)]["x"],
                1.0,
            ]
            for row in selected
        ]
        coefficients, covariance, _ = explicit_ols(
            design, [row["y"] for row in selected]
        )
        expected = [
            float.fromhex("0x1.8ad016f50c0b7p-1"),
            float.fromhex("0x1.30836ede4a100p-1"),
            float.fromhex("0x1.4a29735435d3ep+0"),
        ]
        expected_covariance = [
            [float.fromhex("0x1.34338aa04cc7bp-1"), float.fromhex("0x1.3ca52a93d8c3bp-2"), -float.fromhex("0x1.c16cd24d58b94p-1")],
            [float.fromhex("0x1.3ca52a93d8c3bp-2"), float.fromhex("0x1.f013d2eb5c228p-2"), -float.fromhex("0x1.4ae4eacc24d88p-1")],
            [-float.fromhex("0x1.c16cd24d58b94p-1"), -float.fromhex("0x1.4ae4eacc24d88p-1"), float.fromhex("0x1.79f87704f1cd4p+0")],
        ]
        for actual, target in zip(coefficients, expected):
            self.assertTrue(math.isclose(actual, target, rel_tol=1e-7, abs_tol=1e-10))
        for actual_row, target_row in zip(covariance, expected_covariance):
            for actual, target in zip(actual_row, target_row):
                self.assertTrue(math.isclose(actual, target, rel_tol=1e-6, abs_tol=1e-10))


if __name__ == "__main__":
    unittest.main()
