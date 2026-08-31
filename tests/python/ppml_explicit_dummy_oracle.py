"""Small, dependency-free PPML oracle based on explicit design matrices.

This module deliberately does not import fereg production code.  It is meant
to provide a readable mathematical reference for small fixtures and captured
Stata results.  Fixed effects must be expanded into explicit dummy columns by
the caller, so every rank and normalization choice is visible in the fixture.

The covariance kernel is asymptotic by default.  A caller may supply a known
finite-sample correction as ``covariance_correction``; choosing that policy is
intentionally outside this independent numerical oracle.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Hashable, Sequence


Matrix = list[list[float]]
Vector = list[float]


def _as_float_vector(values: Sequence[float], *, name: str) -> Vector:
    result = [float(value) for value in values]
    if not result:
        raise ValueError(f"{name} must not be empty")
    if any(not math.isfinite(value) for value in result):
        raise ValueError(f"{name} must be finite")
    return result


def _validate_design(design: Sequence[Sequence[float]], rows: int) -> Matrix:
    result = [[float(value) for value in row] for row in design]
    if len(result) != rows:
        raise ValueError("design and outcome lengths differ")
    if not result or not result[0]:
        raise ValueError("design must have at least one row and one column")
    columns = len(result[0])
    if any(len(row) != columns for row in result):
        raise ValueError("design is ragged")
    if any(not math.isfinite(value) for row in result for value in row):
        raise ValueError("design must be finite")
    return result


def transpose(matrix: Sequence[Sequence[float]]) -> Matrix:
    return [list(column) for column in zip(*matrix)]


def matvec(matrix: Sequence[Sequence[float]], vector: Sequence[float]) -> Vector:
    return [sum(left * right for left, right in zip(row, vector)) for row in matrix]


def matmul(left: Sequence[Sequence[float]], right: Sequence[Sequence[float]]) -> Matrix:
    right_t = transpose(right)
    return [
        [sum(a * b for a, b in zip(left_row, right_column)) for right_column in right_t]
        for left_row in left
    ]


def outer(left: Sequence[float], right: Sequence[float]) -> Matrix:
    return [[a * b for b in right] for a in left]


def add_matrix(left: Matrix, right: Sequence[Sequence[float]]) -> None:
    for row, increment in zip(left, right):
        for column, value in enumerate(increment):
            row[column] += value


def scaled_matrix(matrix: Sequence[Sequence[float]], scale: float) -> Matrix:
    return [[scale * value for value in row] for row in matrix]


def invert(matrix: Sequence[Sequence[float]], *, tolerance: float = 1e-13) -> Matrix:
    """Invert a small nonsingular matrix using scaled partial pivoting."""

    size = len(matrix)
    if size == 0 or any(len(row) != size for row in matrix):
        raise ValueError("matrix must be nonempty and square")
    augmented = [
        [float(value) for value in row]
        + [float(row_index == column) for column in range(size)]
        for row_index, row in enumerate(matrix)
    ]
    scale = max(abs(value) for row in matrix for value in row)
    threshold = tolerance * max(1.0, scale)
    for column in range(size):
        pivot = max(range(column, size), key=lambda row: abs(augmented[row][column]))
        if abs(augmented[pivot][column]) <= threshold:
            raise ValueError("matrix is singular at oracle tolerance")
        augmented[column], augmented[pivot] = augmented[pivot], augmented[column]
        pivot_value = augmented[column][column]
        augmented[column] = [value / pivot_value for value in augmented[column]]
        for row in range(size):
            if row == column:
                continue
            factor = augmented[row][column]
            if factor == 0.0:
                continue
            augmented[row] = [
                value - factor * pivot_component
                for value, pivot_component in zip(augmented[row], augmented[column])
            ]
    return [row[size:] for row in augmented]


def select_independent_columns(
    design: Sequence[Sequence[float]], *, tolerance: float = 1e-11
) -> tuple[list[int], list[int]]:
    """Keep the first independent columns using modified Gram--Schmidt.

    The deterministic left-to-right rule makes the omitted-column status part
    of the fixture instead of relying on a linear algebra library's pivoting.
    """

    columns = transpose(design)
    basis: list[Vector] = []
    kept: list[int] = []
    omitted: list[int] = []
    for index, column in enumerate(columns):
        residual = [float(value) for value in column]
        original_norm = math.sqrt(sum(value * value for value in residual))
        if original_norm == 0.0:
            omitted.append(index)
            continue
        # A second orthogonalization pass is cheap for oracle-sized matrices
        # and avoids misclassifying nearly represented columns.
        for _ in range(2):
            for direction in basis:
                projection = sum(value * axis for value, axis in zip(residual, direction))
                residual = [
                    value - projection * axis for value, axis in zip(residual, direction)
                ]
        residual_norm = math.sqrt(sum(value * value for value in residual))
        if residual_norm <= tolerance * max(1.0, original_norm):
            omitted.append(index)
            continue
        basis.append([value / residual_norm for value in residual])
        kept.append(index)
    return kept, omitted


def dummy_columns(
    values: Sequence[Hashable], *, reference: Hashable | None = None
) -> tuple[Matrix, list[Hashable]]:
    """Return reference-coded explicit dummy columns in deterministic order."""

    levels = list(dict.fromkeys(values))
    if not levels:
        raise ValueError("dummy source must not be empty")
    if reference is None:
        reference = levels[0]
    if reference not in levels:
        raise ValueError("dummy reference is not present")
    reported = [level for level in levels if level != reference]
    return (
        [[float(value == level) for level in reported] for value in values],
        reported,
    )


def poisson_deviance(outcome: Sequence[float], mean: Sequence[float], weights: Sequence[float]) -> float:
    total = 0.0
    for y_value, mu_value, weight in zip(outcome, mean, weights):
        contribution = mu_value if y_value == 0.0 else mu_value - y_value + y_value * math.log(y_value / mu_value)
        total += 2.0 * weight * contribution
    return total


def poisson_log_likelihood_kernel(
    outcome: Sequence[float], eta: Sequence[float], mean: Sequence[float], weights: Sequence[float]
) -> float:
    """Return sum w * (y eta - mu), omitting beta-invariant log-factorials."""

    return sum(
        weight * (y_value * eta_value - mu_value)
        for y_value, eta_value, mu_value, weight in zip(outcome, eta, mean, weights)
    )


def poisson_residuals(outcome: Sequence[float], mean: Sequence[float]) -> dict[str, Vector]:
    if len(outcome) != len(mean):
        raise ValueError("outcome and mean lengths differ")
    response: Vector = []
    pearson: Vector = []
    anscombe: Vector = []
    deviance: Vector = []
    working: Vector = []
    for y_value, mu_value in zip(outcome, mean):
        y_value = float(y_value)
        mu_value = float(mu_value)
        if y_value < 0.0 or not math.isfinite(y_value):
            raise ValueError("Poisson outcome must be finite and nonnegative")
        if mu_value <= 0.0 or not math.isfinite(mu_value):
            raise ValueError("Poisson mean must be finite and positive")
        raw = y_value - mu_value
        contribution = (
            mu_value
            if y_value == 0.0
            else mu_value - y_value + y_value * math.log(y_value / mu_value)
        )
        response.append(raw)
        pearson.append(raw / math.sqrt(mu_value))
        anscombe.append(1.5 * (y_value ** (2.0 / 3.0) - mu_value ** (2.0 / 3.0)) / mu_value ** (1.0 / 6.0))
        # ppmlhdfe's predict,deviance returns the nonnegative contribution,
        # despite labeling it a residual; retain that exact observable formula.
        deviance.append(2.0 * contribution)
        working.append(raw / mu_value)
    return {
        "response": response,
        "pearson": pearson,
        "anscombe": anscombe,
        "deviance": deviance,
        "score": response[:],
        "working": working,
    }


def _cross_product(
    design: Sequence[Sequence[float]], diagonal: Sequence[float]
) -> Matrix:
    columns = len(design[0])
    result = [[0.0] * columns for _ in range(columns)]
    for row, weight in zip(design, diagonal):
        for left in range(columns):
            weighted_left = weight * row[left]
            for right in range(columns):
                result[left][right] += weighted_left * row[right]
    return result


def sandwich_covariance(
    design: Sequence[Sequence[float]],
    outcome: Sequence[float],
    mean: Sequence[float],
    prior_weights: Sequence[float],
    *,
    weight_kind: str = "none",
    clusters: Sequence[Hashable] | None = None,
    correction: float = 1.0,
) -> Matrix:
    """Compute the explicit Poisson robust or one-way-cluster sandwich.

    ``weight_kind='fweight'`` treats each integer weight as literal repeated
    independent observations for robust VCE.  With clusters, all copies retain
    their source cluster and therefore contribute to the cluster score sum.
    Probability weights use w in the score and w*mu in the Hessian, making the
    robust and clustered sandwich invariant to a common rescaling of weights.
    """

    if correction <= 0.0 or not math.isfinite(correction):
        raise ValueError("correction must be finite and positive")
    if weight_kind not in {"none", "fweight", "pweight"}:
        raise ValueError("unsupported weight kind")
    if clusters is not None and len(clusters) != len(design):
        raise ValueError("cluster and design lengths differ")

    hessian = _cross_product(
        design,
        [weight * mu_value for weight, mu_value in zip(prior_weights, mean)],
    )
    bread = invert(hessian)
    score_rows = [
        [weight * (y_value - mu_value) * value for value in row]
        for row, y_value, mu_value, weight in zip(design, outcome, mean, prior_weights)
    ]
    columns = len(design[0])
    meat = [[0.0] * columns for _ in range(columns)]
    if clusters is not None:
        group_scores: dict[Hashable, Vector] = {}
        for cluster, score in zip(clusters, score_rows):
            aggregate = group_scores.setdefault(cluster, [0.0] * columns)
            for column, value in enumerate(score):
                aggregate[column] += value
        for aggregate in group_scores.values():
            add_matrix(meat, outer(aggregate, aggregate))
    elif weight_kind == "fweight":
        # score_rows contain f*s.  Literal expansion requires f*s*s', so
        # divide the outer product by f rather than treating f as a pweight.
        for score, weight in zip(score_rows, prior_weights):
            add_matrix(meat, scaled_matrix(outer(score, score), 1.0 / weight))
    else:
        for score in score_rows:
            add_matrix(meat, outer(score, score))
    covariance = matmul(matmul(bread, meat), bread)
    return scaled_matrix(covariance, correction)


@dataclass(frozen=True)
class PpmlOracleResult:
    coefficients: Vector
    compact_coefficients: Vector
    covariance: Matrix
    compact_covariance: Matrix
    eta: Vector
    mean: Vector
    score: Vector
    hessian: Matrix
    deviance: float
    log_likelihood_kernel: float
    iterations: int
    kept_columns: list[int]
    omitted_columns: list[int]
    residuals: dict[str, Vector]


def fit_explicit_ppml(
    design: Sequence[Sequence[float]],
    outcome: Sequence[float],
    *,
    offset: Sequence[float] | None = None,
    exposure: Sequence[float] | None = None,
    weights: Sequence[float] | None = None,
    weight_kind: str = "none",
    vce: str = "iid",
    clusters: Sequence[Hashable] | None = None,
    covariance_correction: float = 1.0,
    tolerance: float = 1e-11,
    max_iterations: int = 200,
) -> PpmlOracleResult:
    """Fit a small full-rank Poisson log-link model by damped Newton steps."""

    y = _as_float_vector(outcome, name="outcome")
    if any(value < 0.0 for value in y):
        raise ValueError("Poisson outcome must be nonnegative")
    if not any(value > 0.0 for value in y):
        raise ValueError("Poisson outcome must contain a positive value")
    x_full = _validate_design(design, len(y))
    if offset is not None and exposure is not None:
        raise ValueError("offset and exposure are mutually exclusive")
    if exposure is not None:
        exposure_values = _as_float_vector(exposure, name="exposure")
        if len(exposure_values) != len(y):
            raise ValueError("exposure and outcome lengths differ")
        if any(value <= 0.0 for value in exposure_values):
            raise ValueError("exposure must be positive")
        offset_values = [math.log(value) for value in exposure_values]
    elif offset is not None:
        offset_values = _as_float_vector(offset, name="offset")
        if len(offset_values) != len(y):
            raise ValueError("offset and outcome lengths differ")
    else:
        offset_values = [0.0] * len(y)

    if weights is None:
        prior_weights = [1.0] * len(y)
    else:
        prior_weights = _as_float_vector(weights, name="weights")
        if len(prior_weights) != len(y):
            raise ValueError("weights and outcome lengths differ")
    if any(value <= 0.0 for value in prior_weights):
        raise ValueError("weights must be positive")
    if weight_kind not in {"none", "fweight", "pweight"}:
        raise ValueError("unsupported weight kind")
    if weight_kind == "none" and weights is not None:
        raise ValueError("explicit weights require fweight or pweight semantics")
    if weight_kind == "fweight" and any(value != math.floor(value) for value in prior_weights):
        raise ValueError("fweights must be integers")
    if vce not in {"iid", "robust", "cluster"}:
        raise ValueError("unsupported VCE")
    if vce == "cluster" and clusters is None:
        raise ValueError("cluster VCE requires cluster identifiers")
    if clusters is not None and len(clusters) != len(y):
        raise ValueError("cluster and outcome lengths differ")

    kept, omitted = select_independent_columns(x_full)
    if not kept:
        raise ValueError("design has no independent columns")
    x = [[row[column] for column in kept] for row in x_full]
    beta = [0.0] * len(kept)

    def state(candidate: Sequence[float]) -> tuple[Vector, Vector, float]:
        linear = matvec(x, candidate)
        eta_values = [base + value for base, value in zip(offset_values, linear)]
        try:
            mean_values = [math.exp(value) for value in eta_values]
        except OverflowError:
            return eta_values, [math.inf] * len(eta_values), -math.inf
        if any(not math.isfinite(value) or value <= 0.0 for value in mean_values):
            return eta_values, mean_values, -math.inf
        objective = poisson_log_likelihood_kernel(y, eta_values, mean_values, prior_weights)
        return eta_values, mean_values, objective

    eta, mean, objective = state(beta)
    converged_iteration: int | None = None
    for iteration in range(1, max_iterations + 1):
        score = [
            sum(
                weight * row[column] * (y_value - mu_value)
                for row, y_value, mu_value, weight in zip(x, y, mean, prior_weights)
            )
            for column in range(len(kept))
        ]
        hessian = _cross_product(
            x,
            [weight * mu_value for weight, mu_value in zip(prior_weights, mean)],
        )
        step = matvec(invert(hessian), score)
        directional_gain = sum(value * delta for value, delta in zip(score, step))
        scale = 1.0
        accepted = False
        while scale >= 2.0 ** -40:
            candidate = [value + scale * delta for value, delta in zip(beta, step)]
            candidate_eta, candidate_mean, candidate_objective = state(candidate)
            if candidate_objective >= objective + 1e-4 * scale * directional_gain:
                beta = candidate
                eta = candidate_eta
                mean = candidate_mean
                objective = candidate_objective
                accepted = True
                break
            scale *= 0.5
        if not accepted:
            raise ArithmeticError("PPML oracle Newton step could not improve the objective")
        scaled_step = max(abs(scale * value) for value in step)
        score_scale = max(
            1.0,
            max(
                sum(weight * mu_value * abs(row[column]) for row, mu_value, weight in zip(x, mean, prior_weights))
                for column in range(len(kept))
            ),
        )
        new_score_max = max(
            abs(
                sum(
                    weight * row[column] * (y_value - mu_value)
                    for row, y_value, mu_value, weight in zip(x, y, mean, prior_weights)
                )
            )
            for column in range(len(kept))
        )
        if scaled_step <= tolerance * (1.0 + max(abs(value) for value in beta)) and new_score_max <= math.sqrt(tolerance) * score_scale:
            converged_iteration = iteration
            break
    if converged_iteration is None:
        raise ArithmeticError("PPML oracle did not converge")

    score = [
        sum(
            weight * row[column] * (y_value - mu_value)
            for row, y_value, mu_value, weight in zip(x, y, mean, prior_weights)
        )
        for column in range(len(kept))
    ]
    hessian = _cross_product(
        x, [weight * mu_value for weight, mu_value in zip(prior_weights, mean)]
    )
    bread = invert(hessian)
    if vce == "iid":
        compact_covariance = scaled_matrix(bread, covariance_correction)
    else:
        compact_covariance = sandwich_covariance(
            x,
            y,
            mean,
            prior_weights,
            weight_kind=weight_kind,
            clusters=clusters if vce == "cluster" else None,
            correction=covariance_correction,
        )
    full_coefficients = [0.0] * len(x_full[0])
    for column, value in zip(kept, beta):
        full_coefficients[column] = value
    full_covariance = [[0.0] * len(x_full[0]) for _ in range(len(x_full[0]))]
    for compact_left, full_left in enumerate(kept):
        for compact_right, full_right in enumerate(kept):
            full_covariance[full_left][full_right] = compact_covariance[compact_left][compact_right]
    return PpmlOracleResult(
        coefficients=full_coefficients,
        compact_coefficients=beta,
        covariance=full_covariance,
        compact_covariance=compact_covariance,
        eta=eta,
        mean=mean,
        score=score,
        hessian=hessian,
        deviance=poisson_deviance(y, mean, prior_weights),
        log_likelihood_kernel=objective,
        iterations=converged_iteration,
        kept_columns=kept,
        omitted_columns=omitted,
        residuals=poisson_residuals(y, mean),
    )


@dataclass(frozen=True)
class SeparationCertificate:
    direction: Vector
    linear_prediction: Vector
    separated_rows: list[int]


def certify_separating_direction(
    design: Sequence[Sequence[float]],
    outcome: Sequence[float],
    direction: Sequence[float],
    *,
    tolerance: float = 1e-12,
) -> SeparationCertificate:
    """Validate a supplied PPML separation direction.

    A valid direction is zero on every positive-outcome observation,
    nonpositive on every zero-outcome observation, and strictly negative for
    at least one zero.  Scaling the coefficient along this direction toward
    positive infinity leaves positive rows unchanged while their separated
    zero-row means approach zero.
    """

    y = _as_float_vector(outcome, name="outcome")
    x = _validate_design(design, len(y))
    gamma = _as_float_vector(direction, name="direction")
    if len(gamma) != len(x[0]):
        raise ValueError("direction and design column counts differ")
    linear = matvec(x, gamma)
    for row, (y_value, value) in enumerate(zip(y, linear)):
        if y_value > 0.0 and abs(value) > tolerance:
            raise ValueError(f"direction changes positive-outcome row {row}")
        if y_value == 0.0 and value > tolerance:
            raise ValueError(f"direction increases zero-outcome row {row}")
    separated = [
        row
        for row, (y_value, value) in enumerate(zip(y, linear))
        if y_value == 0.0 and value < -tolerance
    ]
    if not separated:
        raise ValueError("direction has no strictly separated zero row")
    return SeparationCertificate(gamma, linear, separated)
