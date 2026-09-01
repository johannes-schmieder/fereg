from __future__ import annotations

import math
import unittest

from ppml_explicit_dummy_oracle import (
    fit_explicit_ppml,
)
from ppml_multiway_cluster_oracle import (
    _repair_multiway_psd,
    explicit_multiway_cluster_covariance,
)


Matrix = list[list[float]]


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
            [1.0] * len(self.design),
            self.cluster_dimensions[:1],
        )
        assert_matrix_close(self, covariance, clustered.compact_covariance)
        self.assertEqual(diagnostics, [(1, 10)])

    def test_post_inclusion_psd_repair_clips_a_material_negative_direction(self) -> None:
        raw = [[1.0, 2.0], [2.0, 1.0]]
        repaired = _repair_multiway_psd(raw)
        self.assertNotEqual(repaired, raw)
        self.assertGreaterEqual(
            repaired[0][0] * repaired[1][1] - repaired[0][1] * repaired[1][0],
            -1e-14,
        )
        self.assertAlmostEqual(repaired[0][0], repaired[1][1])

    def test_every_supported_dimension_has_all_nonempty_subsets(self) -> None:
        for dimensions in range(1, 6):
            with self.subTest(dimensions=dimensions):
                covariance, diagnostics = explicit_multiway_cluster_covariance(
                    self.design,
                    self.outcome,
                    self.fit.mean,
                    self.fit.hessian,
                    [1.0] * len(self.design),
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
            [1.0] * len(self.design),
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
                    [1.0] * len(self.design),
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
            [1.0] * len(self.design),
            dimensions,
        )
        minimum_groups = min(len(set(dimension)) for dimension in dimensions)
        correction = minimum_groups / (minimum_groups - 1)
        corrected, _ = explicit_multiway_cluster_covariance(
            self.design,
            self.outcome,
            self.fit.mean,
            self.fit.hessian,
            [1.0] * len(self.design),
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
                [1.0] * len(self.design),
                [],
            )
        with self.assertRaisesRegex(ValueError, "one through five"):
            explicit_multiway_cluster_covariance(
                self.design,
                self.outcome,
                self.fit.mean,
                self.fit.hessian,
                [1.0] * len(self.design),
                [self.cluster_dimensions[0]] * 6,
            )
        with self.assertRaisesRegex(ValueError, "finite and positive"):
            explicit_multiway_cluster_covariance(
                self.design,
                self.outcome,
                self.fit.mean,
                self.fit.hessian,
                [1.0] * len(self.design),
                self.cluster_dimensions[:1],
                correction=0.0,
            )


if __name__ == "__main__":
    unittest.main()
