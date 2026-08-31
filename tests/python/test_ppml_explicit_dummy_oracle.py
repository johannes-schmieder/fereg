from __future__ import annotations

import math
import unittest

from ppml_explicit_dummy_oracle import (
    certify_separating_direction,
    dummy_columns,
    fit_explicit_ppml,
    invert,
    matmul,
    outer,
    poisson_residuals,
    sandwich_covariance,
)


def assert_vector_close(
    case: unittest.TestCase,
    actual: list[float],
    expected: list[float],
    *,
    tolerance: float = 1e-9,
) -> None:
    case.assertEqual(len(actual), len(expected))
    for index, (left, right) in enumerate(zip(actual, expected)):
        case.assertTrue(
            math.isclose(left, right, rel_tol=tolerance, abs_tol=tolerance),
            f"element {index}: {left!r} != {right!r}",
        )


def assert_matrix_close(
    case: unittest.TestCase,
    actual: list[list[float]],
    expected: list[list[float]],
    *,
    tolerance: float = 1e-9,
) -> None:
    case.assertEqual(len(actual), len(expected))
    for row, (left, right) in enumerate(zip(actual, expected)):
        try:
            assert_vector_close(case, left, right, tolerance=tolerance)
        except AssertionError as error:
            raise AssertionError(f"matrix row {row}: {error}") from error


def exact_cell_mean_fixture(
    cells: list[tuple[int, ...]], coefficients: list[float]
) -> tuple[list[list[float]], list[float]]:
    """Repeat each cell twice with counts bracketing its integer mean."""

    design: list[list[float]] = []
    outcome: list[float] = []
    for cell in cells:
        row = [1.0, *map(float, cell)]
        mean = math.exp(sum(value * beta for value, beta in zip(row, coefficients)))
        rounded = round(mean)
        if not math.isclose(mean, rounded, abs_tol=1e-12):
            raise AssertionError("test cell mean must be integral")
        design.extend([row[:], row[:]])
        outcome.extend([float(rounded - 1), float(rounded + 1)])
    return design, outcome


class ExplicitDesignPpmlTests(unittest.TestCase):
    def test_no_fixed_effect_matches_closed_form_group_means(self) -> None:
        design = [[1.0, 0.0], [1.0, 0.0], [1.0, 1.0], [1.0, 1.0]]
        outcome = [1.0, 3.0, 2.0, 6.0]
        result = fit_explicit_ppml(design, outcome)
        assert_vector_close(self, result.coefficients, [math.log(2.0), math.log(2.0)])
        assert_vector_close(self, result.mean, [2.0, 2.0, 4.0, 4.0])
        assert_vector_close(self, result.score, [0.0, 0.0])

    def test_one_fixed_effect_uses_visible_reference_dummy(self) -> None:
        cells = [(x, group) for group in (0, 1) for x in (0, 1)]
        expected = [0.0, math.log(2.0), math.log(3.0)]
        design, outcome = exact_cell_mean_fixture(cells, expected)
        result = fit_explicit_ppml(design, outcome)
        assert_vector_close(self, result.coefficients, expected)
        self.assertEqual(result.omitted_columns, [])

    def test_multiway_fixed_effects_are_ordinary_explicit_columns(self) -> None:
        cells = [
            (x, first_effect, second_effect)
            for first_effect in (0, 1)
            for second_effect in (0, 1)
            for x in (0, 1)
        ]
        expected = [0.0, math.log(2.0), math.log(2.0), math.log(2.0)]
        design, outcome = exact_cell_mean_fixture(cells, expected)
        result = fit_explicit_ppml(design, outcome)
        assert_vector_close(self, result.coefficients, expected)
        self.assertAlmostEqual(result.deviance, sum(result.residuals["deviance"]), places=9)

    def test_dummy_helper_reports_reference_coding(self) -> None:
        columns, levels = dummy_columns(["b", "a", "c", "b"], reference="b")
        self.assertEqual(levels, ["a", "c"])
        self.assertEqual(columns, [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]])

    def test_rank_policy_keeps_first_column_and_zero_fills_omitted_status(self) -> None:
        outcome = [1.0, 3.0, 2.0, 6.0, 3.0, 9.0]
        x_values = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0]
        collinear = [[1.0, value, 2.0 * value] for value in x_values]
        reduced = [[1.0, value] for value in x_values]
        result = fit_explicit_ppml(collinear, outcome)
        reference = fit_explicit_ppml(reduced, outcome)
        self.assertEqual(result.kept_columns, [0, 1])
        self.assertEqual(result.omitted_columns, [2])
        assert_vector_close(self, result.coefficients[:2], reference.coefficients)
        self.assertEqual(result.coefficients[2], 0.0)
        self.assertTrue(all(result.covariance[2][column] == 0.0 for column in range(3)))
        self.assertTrue(all(result.covariance[row][2] == 0.0 for row in range(3)))


class OffsetExposureAndWeightTests(unittest.TestCase):
    def test_exposure_is_exactly_logged_offset(self) -> None:
        design = [[1.0, 0.0], [1.0, 0.0], [1.0, 1.0], [1.0, 1.0]]
        exposure = [1.0, 2.0, 1.0, 2.0]
        outcome = [1.0, 2.0, 2.0, 4.0]
        by_exposure = fit_explicit_ppml(design, outcome, exposure=exposure)
        by_offset = fit_explicit_ppml(design, outcome, offset=[math.log(value) for value in exposure])
        assert_vector_close(self, by_exposure.coefficients, [0.0, math.log(2.0)])
        assert_vector_close(self, by_exposure.coefficients, by_offset.coefficients)
        assert_vector_close(self, by_exposure.mean, exposure[:2] + [2.0, 4.0])
        with self.assertRaisesRegex(ValueError, "mutually exclusive"):
            fit_explicit_ppml(design, outcome, offset=[0.0] * 4, exposure=exposure)
        with self.assertRaisesRegex(ValueError, "positive"):
            fit_explicit_ppml(design, outcome, exposure=[1.0, 0.0, 1.0, 2.0])

    def test_fweight_fit_and_robust_vce_equal_literal_expansion(self) -> None:
        design = [[1.0, 0.0], [1.0, 1.0], [1.0, 2.0], [1.0, 3.0]]
        outcome = [1.0, 2.0, 7.0, 5.0]
        weights = [1.0, 3.0, 2.0, 4.0]
        weighted = fit_explicit_ppml(
            design,
            outcome,
            weights=weights,
            weight_kind="fweight",
            vce="robust",
        )
        expanded_design: list[list[float]] = []
        expanded_outcome: list[float] = []
        for row, value, frequency in zip(design, outcome, weights):
            expanded_design.extend([row[:]] * int(frequency))
            expanded_outcome.extend([value] * int(frequency))
        expanded = fit_explicit_ppml(expanded_design, expanded_outcome, vce="robust")
        assert_vector_close(self, weighted.coefficients, expanded.coefficients)
        assert_matrix_close(self, weighted.covariance, expanded.covariance)

    def test_pweight_robust_vce_is_invariant_to_common_rescaling(self) -> None:
        design = [[1.0, 0.0], [1.0, 1.0], [1.0, 2.0], [1.0, 3.0], [1.0, 4.0]]
        outcome = [1.0, 4.0, 2.0, 8.0, 7.0]
        weights = [0.5, 2.0, 1.25, 3.0, 0.75]
        original = fit_explicit_ppml(
            design, outcome, weights=weights, weight_kind="pweight", vce="robust"
        )
        rescaled = fit_explicit_ppml(
            design,
            outcome,
            weights=[17.0 * value for value in weights],
            weight_kind="pweight",
            vce="robust",
        )
        assert_vector_close(self, original.coefficients, rescaled.coefficients)
        assert_matrix_close(self, original.covariance, rescaled.covariance)


class SandwichOracleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.design = [
            [1.0, 0.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [1.0, 3.0],
            [1.0, 4.0],
            [1.0, 5.0],
        ]
        self.outcome = [1.0, 4.0, 2.0, 8.0, 7.0, 14.0]

    def test_robust_sandwich_matches_direct_score_outer_products(self) -> None:
        result = fit_explicit_ppml(self.design, self.outcome, vce="robust")
        bread = invert(result.hessian)
        score_rows = [
            [(y_value - mu_value) * value for value in row]
            for row, y_value, mu_value in zip(self.design, self.outcome, result.mean)
        ]
        meat = [[0.0, 0.0], [0.0, 0.0]]
        for score in score_rows:
            contribution = outer(score, score)
            for left in range(2):
                for right in range(2):
                    meat[left][right] += contribution[left][right]
        expected = matmul(matmul(bread, meat), bread)
        assert_matrix_close(self, result.compact_covariance, expected)

    def test_cluster_sandwich_aggregates_scores_before_outer_product(self) -> None:
        clusters = ["a", "a", "b", "b", "c", "c"]
        result = fit_explicit_ppml(
            self.design, self.outcome, vce="cluster", clusters=clusters
        )
        bread = invert(result.hessian)
        grouped: dict[str, list[float]] = {}
        for cluster, row, y_value, mu_value in zip(
            clusters, self.design, self.outcome, result.mean
        ):
            score = grouped.setdefault(cluster, [0.0, 0.0])
            for column, value in enumerate(row):
                score[column] += value * (y_value - mu_value)
        meat = [[0.0, 0.0], [0.0, 0.0]]
        for score in grouped.values():
            contribution = outer(score, score)
            for left in range(2):
                for right in range(2):
                    meat[left][right] += contribution[left][right]
        expected = matmul(matmul(bread, meat), bread)
        assert_matrix_close(self, result.compact_covariance, expected)

    def test_cluster_fweights_equal_expansion_with_preserved_clusters(self) -> None:
        clusters = ["a", "a", "b", "c", "c", "d"]
        weights = [1.0, 2.0, 3.0, 1.0, 2.0, 1.0]
        weighted = fit_explicit_ppml(
            self.design,
            self.outcome,
            weights=weights,
            weight_kind="fweight",
            vce="cluster",
            clusters=clusters,
        )
        expanded_design: list[list[float]] = []
        expanded_outcome: list[float] = []
        expanded_clusters: list[str] = []
        for row, y_value, cluster, frequency in zip(
            self.design, self.outcome, clusters, weights
        ):
            expanded_design.extend([row[:]] * int(frequency))
            expanded_outcome.extend([y_value] * int(frequency))
            expanded_clusters.extend([cluster] * int(frequency))
        expanded = fit_explicit_ppml(
            expanded_design,
            expanded_outcome,
            vce="cluster",
            clusters=expanded_clusters,
        )
        assert_vector_close(self, weighted.coefficients, expanded.coefficients)
        assert_matrix_close(self, weighted.covariance, expanded.covariance)

    def test_finite_sample_multiplier_is_explicit_and_only_scales_covariance(self) -> None:
        base = fit_explicit_ppml(self.design, self.outcome, vce="robust")
        corrected = fit_explicit_ppml(
            self.design,
            self.outcome,
            vce="robust",
            covariance_correction=1.75,
        )
        assert_vector_close(self, base.coefficients, corrected.coefficients)
        assert_matrix_close(
            self,
            corrected.covariance,
            [[1.75 * value for value in row] for row in base.covariance],
        )


class ResidualAndSeparationOracleTests(unittest.TestCase):
    def test_all_pinned_ppmlhdfe_residual_formulas_are_explicit(self) -> None:
        outcome = [0.0, 1.0, 4.0]
        mean = [0.5, 1.5, 3.0]
        residuals = poisson_residuals(outcome, mean)
        raw = [-0.5, -0.5, 1.0]
        assert_vector_close(self, residuals["response"], raw)
        assert_vector_close(self, residuals["score"], raw)
        assert_vector_close(
            self,
            residuals["pearson"],
            [raw_value / math.sqrt(mu_value) for raw_value, mu_value in zip(raw, mean)],
        )
        assert_vector_close(
            self,
            residuals["working"],
            [raw_value / mu_value for raw_value, mu_value in zip(raw, mean)],
        )
        assert_vector_close(
            self,
            residuals["anscombe"],
            [
                1.5 * (y_value ** (2.0 / 3.0) - mu_value ** (2.0 / 3.0)) / mu_value ** (1.0 / 6.0)
                for y_value, mu_value in zip(outcome, mean)
            ],
        )
        assert_vector_close(
            self,
            residuals["deviance"],
            [
                1.0,
                2.0 * (1.5 - 1.0 + math.log(1.0 / 1.5)),
                2.0 * (3.0 - 4.0 + 4.0 * math.log(4.0 / 3.0)),
            ],
        )

    def test_zero_only_dummy_has_constructive_separation_certificate(self) -> None:
        # The last column selects a fixed-effect level observed only with y=0.
        design = [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ]
        outcome = [1.0, 2.0, 3.0, 4.0, 0.0, 0.0]
        certificate = certify_separating_direction(
            design, outcome, [0.0, 0.0, -1.0]
        )
        self.assertEqual(certificate.separated_rows, [4, 5])
        retained = [row for index, row in enumerate(design) if index not in certificate.separated_rows]
        retained_outcome = [
            value for index, value in enumerate(outcome) if index not in certificate.separated_rows
        ]
        fitted = fit_explicit_ppml([row[:2] for row in retained], retained_outcome)
        self.assertTrue(all(math.isfinite(value) for value in fitted.coefficients))

    def test_invalid_separation_direction_is_rejected(self) -> None:
        design = [[1.0, 0.0], [1.0, 1.0], [1.0, 1.0]]
        outcome = [1.0, 2.0, 0.0]
        with self.assertRaisesRegex(ValueError, "positive-outcome"):
            certify_separating_direction(design, outcome, [0.0, -1.0])
        with self.assertRaisesRegex(ValueError, "strictly separated"):
            certify_separating_direction(design, outcome, [0.0, 0.0])

    def test_domain_boundaries_fail_before_numerical_iteration(self) -> None:
        design = [[1.0], [1.0]]
        with self.assertRaisesRegex(ValueError, "nonnegative"):
            fit_explicit_ppml(design, [1.0, -1.0])
        with self.assertRaisesRegex(ValueError, "positive value"):
            fit_explicit_ppml(design, [0.0, 0.0])
        with self.assertRaisesRegex(ValueError, "fweights must be integers"):
            fit_explicit_ppml(
                design, [1.0, 2.0], weights=[1.0, 1.5], weight_kind="fweight"
            )


if __name__ == "__main__":
    unittest.main()
