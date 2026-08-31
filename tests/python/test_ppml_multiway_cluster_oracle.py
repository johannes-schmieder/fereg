from __future__ import annotations

import math
import unittest
from collections.abc import Hashable, Sequence

from ppml_explicit_dummy_oracle import (
    fit_explicit_ppml,
    invert,
    matmul,
    outer,
)


Matrix = list[list[float]]


def add_scaled(target: Matrix, source: Matrix, scale: float) -> None:
    for row, increment in zip(target, source):
        for column, value in enumerate(increment):
            row[column] += scale * value


def explicit_multiway_cluster_covariance(
    design: Sequence[Sequence[float]],
    outcome: Sequence[float],
    mean: Sequence[float],
    hessian: Matrix,
    cluster_dimensions: Sequence[Sequence[Hashable]],
    *,
    correction: float = 1.0,
) -> tuple[Matrix, list[tuple[int, int]]]:
    """Independent Cameron--Gelbach--Miller inclusion--exclusion oracle.

    This intentionally does not import a production fereg covariance path.
    The returned diagnostics contain (subset mask, distinct group count) in
    deterministic mask order, which mirrors the observable contract expected
    from the Stata subset table.
    """

    rows = len(design)
    columns = len(design[0])
    dimensions = len(cluster_dimensions)
    if not 1 <= dimensions <= 5:
        raise ValueError("the PPML contract supports one through five dimensions")
    if any(len(dimension) != rows for dimension in cluster_dimensions):
        raise ValueError("cluster dimension and design lengths differ")
    if not math.isfinite(correction) or correction <= 0.0:
        raise ValueError("cluster correction must be finite and positive")

    score_rows = [
        [(y_value - mu_value) * value for value in row]
        for row, y_value, mu_value in zip(design, outcome, mean)
    ]
    meat = [[0.0] * columns for _ in range(columns)]
    diagnostics: list[tuple[int, int]] = []

    for mask in range(1, 1 << dimensions):
        selected = [index for index in range(dimensions) if mask & (1 << index)]
        grouped: dict[tuple[Hashable, ...], list[float]] = {}
        for row_index, score in enumerate(score_rows):
            key = tuple(cluster_dimensions[index][row_index] for index in selected)
            aggregate = grouped.setdefault(key, [0.0] * columns)
            for column, value in enumerate(score):
                aggregate[column] += value

        subset_meat = [[0.0] * columns for _ in range(columns)]
        for aggregate in grouped.values():
            add_scaled(subset_meat, outer(aggregate, aggregate), 1.0)
        sign = 1.0 if len(selected) % 2 else -1.0
        add_scaled(meat, subset_meat, sign)
        diagnostics.append((mask, len(grouped)))

    bread = invert(hessian)
    covariance = matmul(matmul(bread, meat), bread)
    return [[correction * value for value in row] for row in covariance], diagnostics


def assert_matrix_close(
    case: unittest.TestCase,
    actual: Matrix,
    expected: Matrix,
    *,
    tolerance: float = 1e-9,
) -> None:
    case.assertEqual(len(actual), len(expected))
    for row, (left, right) in enumerate(zip(actual, expected)):
        case.assertEqual(len(left), len(right))
        for column, (observed, target) in enumerate(zip(left, right)):
            case.assertTrue(
                math.isclose(observed, target, rel_tol=tolerance, abs_tol=tolerance),
                f"({row}, {column}): {observed!r} != {target!r}",
            )


class MultiwayClusterOracleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.design = [[1.0, (index - 30.0) / 11.0] for index in range(60)]
        cls.outcome = [0.5 + ((7 * index) % 17) for index in range(60)]
        cls.fit = fit_explicit_ppml(cls.design, cls.outcome)
        cls.cluster_dimensions = [
            [index % 10 for index in range(60)],
            [(index // 2) % 9 for index in range(60)],
            [(index // 3) % 8 for index in range(60)],
            [(index // 5) % 7 for index in range(60)],
            [(index // 7) % 6 for index in range(60)],
        ]

    def test_one_way_matches_the_existing_independent_oracle(self) -> None:
        clustered = fit_explicit_ppml(
            self.design,
            self.outcome,
            vce="cluster",
            clusters=self.cluster_dimensions[0],
        )
        covariance, diagnostics = explicit_multiway_cluster_covariance(
            self.design,
            self.outcome,
            self.fit.mean,
            self.fit.hessian,
            self.cluster_dimensions[:1],
        )
        assert_matrix_close(self, covariance, clustered.compact_covariance)
        self.assertEqual(diagnostics, [(1, 10)])

    def test_every_supported_dimension_has_all_nonempty_subsets(self) -> None:
        for dimensions in range(1, 6):
            with self.subTest(dimensions=dimensions):
                covariance, diagnostics = explicit_multiway_cluster_covariance(
                    self.design,
                    self.outcome,
                    self.fit.mean,
                    self.fit.hessian,
                    self.cluster_dimensions[:dimensions],
                )
                self.assertEqual([mask for mask, _ in diagnostics], list(range(1, 2**dimensions)))
                self.assertTrue(all(groups > 1 for _, groups in diagnostics))
                self.assertTrue(all(math.isfinite(value) for row in covariance for value in row))
                for row in range(len(covariance)):
                    for column in range(len(covariance)):
                        self.assertAlmostEqual(covariance[row][column], covariance[column][row])

    def test_repeated_identical_dimensions_collapse_to_one_way(self) -> None:
        one_way, _ = explicit_multiway_cluster_covariance(
            self.design,
            self.outcome,
            self.fit.mean,
            self.fit.hessian,
            self.cluster_dimensions[:1],
        )
        repeated = [self.cluster_dimensions[0]] * 5
        for dimensions in range(2, 6):
            with self.subTest(dimensions=dimensions):
                covariance, diagnostics = explicit_multiway_cluster_covariance(
                    self.design,
                    self.outcome,
                    self.fit.mean,
                    self.fit.hessian,
                    repeated[:dimensions],
                )
                assert_matrix_close(self, covariance, one_way)
                self.assertEqual(len(diagnostics), 2**dimensions - 1)

    def test_common_minimum_group_correction_is_applied_once(self) -> None:
        dimensions = self.cluster_dimensions
        asymptotic, _ = explicit_multiway_cluster_covariance(
            self.design,
            self.outcome,
            self.fit.mean,
            self.fit.hessian,
            dimensions,
        )
        minimum_groups = min(len(set(dimension)) for dimension in dimensions)
        correction = minimum_groups / (minimum_groups - 1)
        corrected, _ = explicit_multiway_cluster_covariance(
            self.design,
            self.outcome,
            self.fit.mean,
            self.fit.hessian,
            dimensions,
            correction=correction,
        )
        assert_matrix_close(
            self,
            corrected,
            [[correction * value for value in row] for row in asymptotic],
        )

    def test_dimension_domain_is_explicit(self) -> None:
        with self.assertRaisesRegex(ValueError, "one through five"):
            explicit_multiway_cluster_covariance(
                self.design,
                self.outcome,
                self.fit.mean,
                self.fit.hessian,
                [],
            )
        with self.assertRaisesRegex(ValueError, "one through five"):
            explicit_multiway_cluster_covariance(
                self.design,
                self.outcome,
                self.fit.mean,
                self.fit.hessian,
                [self.cluster_dimensions[0]] * 6,
            )
        with self.assertRaisesRegex(ValueError, "finite and positive"):
            explicit_multiway_cluster_covariance(
                self.design,
                self.outcome,
                self.fit.mean,
                self.fit.hessian,
                self.cluster_dimensions[:1],
                correction=0.0,
            )


if __name__ == "__main__":
    unittest.main()
