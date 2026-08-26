#!/usr/bin/env python3
"""Generate and verify path-neutral high-precision FE projection fixtures.

The production solvers are deliberately not imported here.  This module builds
the explicit absorbed-design matrix, chooses a deterministic independent basis,
and solves the weighted normal equations using :mod:`decimal` arithmetic.  The
sealed JSON output is therefore an independent observation-space oracle for
bounded weak-graph qualification cases.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
from decimal import Decimal, localcontext
from pathlib import Path
from typing import Any, Iterable


PRECISION = 100
ZERO = Decimal(0)
ONE = Decimal(1)
RANK_THRESHOLD = Decimal("1e-70")
SCHEMA = "fereg.weak_graph_decimal_oracle.v1"


class OracleError(RuntimeError):
    """Raised when a fixture is malformed or the independent solve fails."""


def _d(value: Any) -> Decimal:
    return value if isinstance(value, Decimal) else Decimal(str(value))


def _decimal_text(value: Decimal) -> str:
    if value.is_zero():
        return "0"
    return format(value, ".70E")


def _weighted_dot(left: Iterable[Decimal], right: Iterable[Decimal], weights: list[Decimal]) -> Decimal:
    total = ZERO
    correction = ZERO
    for lhs, rhs, weight in zip(left, right, weights):
        term = weight * lhs * rhs
        adjusted = term - correction
        updated = total + adjusted
        correction = (updated - total) - adjusted
        total = updated
    return total


def _fixture(
    fixture_id: str,
    effects: list[dict[str, Any]],
    rhs: list[list[Any]],
    weights: list[Any] | None = None,
    tags: list[str] | None = None,
) -> dict[str, Any]:
    nobs = len(rhs[0])
    if any(len(column) != nobs for column in rhs):
        raise OracleError(f"{fixture_id}: inconsistent RHS lengths")
    if any(len(effect["codes"]) != nobs for effect in effects):
        raise OracleError(f"{fixture_id}: inconsistent effect lengths")
    return {
        "id": fixture_id,
        "tags": tags or [],
        "effects": effects,
        "weights": [str(value) for value in (weights or [1] * nobs)],
        "rhs": [[str(value) for value in column] for column in rhs],
    }


def fixtures() -> list[dict[str, Any]]:
    """Return deterministic calibration fixtures without using random state."""

    chain_worker: list[int] = []
    chain_firm: list[int] = []
    for worker in range(8):
        chain_worker.extend([worker, worker])
        chain_firm.extend([worker, worker + 1])
    chain_y = [((index * 17) % 23) - 11 for index in range(len(chain_worker))]

    # Two dense lobes connected by a single mobility observation.
    barbell_edges = [
        (0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1),
        (2, 2),
        (3, 2), (3, 3), (4, 2), (4, 3), (5, 2), (5, 3),
    ]
    barbell_worker = [edge[0] for edge in barbell_edges]
    barbell_firm = [edge[1] for edge in barbell_edges]
    barbell_y = [((index * index + 3 * index + 7) % 19) - 9 for index in range(len(barbell_edges))]

    disconnected_edges = [
        (0, 0), (0, 1), (1, 1), (1, 2),
        (2, 3), (2, 4), (3, 4), (3, 5),
    ]
    disconnected_y = [7, -3, 11, 2, -5, 13, 1, -9]

    high_edges = [(worker, firm) for worker in range(5) for firm in range(4)]
    high_y = [((11 * index + 5) % 29) - 14 for index in range(len(high_edges))]

    low_edges: list[tuple[int, int]] = []
    for worker in range(10):
        low_edges.extend([(worker, worker), (worker, worker), (worker, (worker + 1) % 10)])
    low_y = [((13 * index + index * index) % 31) - 15 for index in range(len(low_edges))]
    low_x = [((7 * index + 2) % 17) - 8 for index in range(len(low_edges))]
    unequal_weights = ["0.125", "0.5", "1.25", "3.5", "7.75"] * 6

    slope_groups = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3]
    slope_time = ["-1", "0.25", "2", "-2", "0.5", "3", "-1.5", "1", "2.5", "-3", "0.75", "4"]
    slope_other = [0, 1, 0, 1, 0, 1, 2, 2, 3, 3, 2, 3]
    slope_y = ["2.25", "-1.5", "4.75", "0.5", "3.25", "-2", "1.125", "5", "-3.75", "2", "0.25", "6"]

    def intercept(codes: list[int]) -> dict[str, Any]:
        return {"codes": codes, "intercept": True, "slopes": []}

    return [
        _fixture(
            "chain_two_way",
            [intercept(chain_worker), intercept(chain_firm)],
            [chain_y],
            tags=["chain", "weak_graph"],
        ),
        _fixture(
            "barbell_bridge",
            [intercept(barbell_worker), intercept(barbell_firm)],
            [barbell_y],
            tags=["barbell", "weak_graph"],
        ),
        _fixture(
            "disconnected_components",
            [
                intercept([edge[0] for edge in disconnected_edges]),
                intercept([edge[1] for edge in disconnected_edges]),
            ],
            [disconnected_y],
            tags=["disconnected", "two_components"],
        ),
        _fixture(
            "akm_high_mobility",
            [intercept([edge[0] for edge in high_edges]), intercept([edge[1] for edge in high_edges])],
            [high_y],
            tags=["akm", "high_mobility"],
        ),
        _fixture(
            "akm_low_mobility_weighted_multi_rhs",
            [intercept([edge[0] for edge in low_edges]), intercept([edge[1] for edge in low_edges])],
            [low_y, low_x],
            weights=unequal_weights,
            tags=["akm", "low_mobility", "unequal_weights", "multiple_rhs"],
        ),
        _fixture(
            "absorbed_slope",
            [
                {"codes": slope_groups, "intercept": True, "slopes": [slope_time]},
                intercept(slope_other),
            ],
            [slope_y],
            weights=["0.25", "1", "2.5", "4"] * 3,
            tags=["absorbed_slope", "unequal_weights"],
        ),
    ]


def _explicit_columns(fixture: dict[str, Any]) -> tuple[list[list[Decimal]], list[str]]:
    nobs = len(fixture["weights"])
    columns: list[list[Decimal]] = []
    labels: list[str] = []
    for effect_index, effect in enumerate(fixture["effects"]):
        codes = [int(code) for code in effect["codes"]]
        if any(code < 0 for code in codes):
            raise OracleError(f"{fixture['id']}: negative categorical code")
        levels = max(codes, default=-1) + 1
        if effect.get("intercept", False):
            for level in range(levels):
                columns.append([ONE if code == level else ZERO for code in codes])
                labels.append(f"effect{effect_index}:intercept:{level}")
        for slope_index, raw_slope in enumerate(effect.get("slopes", [])):
            slope = [_d(value) for value in raw_slope]
            if len(slope) != nobs:
                raise OracleError(f"{fixture['id']}: slope length mismatch")
            for level in range(levels):
                columns.append([value if code == level else ZERO for code, value in zip(codes, slope)])
                labels.append(f"effect{effect_index}:slope{slope_index}:{level}")
    return columns, labels


def _independent_columns(
    columns: list[list[Decimal]], weights: list[Decimal]
) -> tuple[list[int], list[list[Decimal]]]:
    selected: list[int] = []
    orthonormal: list[list[Decimal]] = []
    largest_norm = ZERO
    for index, column in enumerate(columns):
        original_norm_square = _weighted_dot(column, column, weights)
        if original_norm_square <= ZERO:
            continue
        original_norm = original_norm_square.sqrt()
        largest_norm = max(largest_norm, original_norm)
        residual = column.copy()
        # Two passes make the rank decision insensitive to classical
        # Gram--Schmidt cancellation on the deliberately weak fixtures.
        for _ in range(2):
            for basis in orthonormal:
                coefficient = _weighted_dot(basis, residual, weights)
                for row, value in enumerate(basis):
                    residual[row] -= coefficient * value
        residual_norm_square = _weighted_dot(residual, residual, weights)
        threshold = max(ONE, largest_norm) * RANK_THRESHOLD
        if residual_norm_square <= threshold * threshold:
            continue
        residual_norm = residual_norm_square.sqrt()
        orthonormal.append([value / residual_norm for value in residual])
        selected.append(index)
    if not selected:
        raise OracleError("explicit absorbed design has no identified column")
    return selected, orthonormal


def _solve_dense(matrix: list[list[Decimal]], rhs: list[Decimal]) -> list[Decimal]:
    n = len(rhs)
    augmented = [row.copy() + [value] for row, value in zip(matrix, rhs)]
    for column in range(n):
        pivot = max(range(column, n), key=lambda row: abs(augmented[row][column]))
        pivot_value = augmented[pivot][column]
        if abs(pivot_value) <= RANK_THRESHOLD:
            raise OracleError("high-precision normal equations lost rank")
        augmented[column], augmented[pivot] = augmented[pivot], augmented[column]
        scale = augmented[column][column]
        for index in range(column, n + 1):
            augmented[column][index] /= scale
        for row in range(n):
            if row == column:
                continue
            multiplier = augmented[row][column]
            if multiplier == ZERO:
                continue
            for index in range(column, n + 1):
                augmented[row][index] -= multiplier * augmented[column][index]
    return [augmented[row][n] for row in range(n)]


def _projection(
    basis: list[list[Decimal]], weights: list[Decimal], values: list[Decimal]
) -> list[Decimal]:
    size = len(basis)
    gram = [[_weighted_dot(basis[row], basis[column], weights) for column in range(size)] for row in range(size)]
    moments = [_weighted_dot(column, values, weights) for column in basis]
    coefficients = _solve_dense(gram, moments)
    return [sum((coefficient * column[row] for coefficient, column in zip(coefficients, basis)), ZERO) for row in range(len(values))]


def solve_fixture(fixture: dict[str, Any]) -> dict[str, Any]:
    with localcontext() as context:
        context.prec = PRECISION
        weights = [_d(value) for value in fixture["weights"]]
        if any(weight <= ZERO for weight in weights):
            raise OracleError(f"{fixture['id']}: weights must be positive")
        columns, labels = _explicit_columns(fixture)
        selected, _ = _independent_columns(columns, weights)
        basis = [columns[index] for index in selected]
        output_rhs: list[dict[str, Any]] = []
        for raw_values in fixture["rhs"]:
            values = [_d(value) for value in raw_values]
            fitted = _projection(basis, weights, values)
            residual = [value - fit for value, fit in zip(values, fitted)]
            objective = _weighted_dot(residual, residual, weights)
            remaining = _projection(basis, weights, residual)
            value_norm = _weighted_dot(values, values, weights).sqrt()
            projection_norm = _weighted_dot(remaining, remaining, weights).sqrt()
            audit = projection_norm / max(ONE, value_norm)
            output_rhs.append(
                {
                    "fitted": [_decimal_text(value) for value in fitted],
                    "residual": [_decimal_text(value) for value in residual],
                    "objective": _decimal_text(objective),
                    "observation_projection_relative_two_norm": _decimal_text(audit),
                }
            )
        return {
            "id": fixture["id"],
            "tags": fixture["tags"],
            "nobs": len(weights),
            "rank": len(selected),
            "basis_labels": [labels[index] for index in selected],
            "weights": fixture["weights"],
            "effects": fixture["effects"],
            "input_rhs": fixture["rhs"],
            "rhs": output_rhs,
        }


def _checksum(payload: dict[str, Any]) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def flat_csv(payload: dict[str, Any]) -> str:
    """Return a dependency-free row format consumed by Rust qualification tests."""

    destination = io.StringIO(newline="")
    fieldnames = [
        "fixture",
        "observation",
        "weight",
        "effect0_code",
        "effect0_intercept",
        "effect0_slope0",
        "effect1_code",
        "effect1_intercept",
        "effect1_slope0",
        "rhs0_input",
        "rhs0_fitted",
        "rhs0_residual",
        "rhs0_objective",
        "rhs1_input",
        "rhs1_fitted",
        "rhs1_residual",
        "rhs1_objective",
    ]
    writer = csv.DictWriter(destination, fieldnames=fieldnames, lineterminator="\n")
    writer.writeheader()
    for fixture in payload["fixtures"]:
        if len(fixture["effects"]) != 2 or len(fixture["rhs"]) > 2:
            raise OracleError(f"{fixture['id']}: flat Rust fixture envelope exceeded")
        for observation in range(fixture["nobs"]):
            row: dict[str, Any] = {
                "fixture": fixture["id"],
                "observation": observation,
                "weight": fixture["weights"][observation],
            }
            for effect_index, effect in enumerate(fixture["effects"]):
                row[f"effect{effect_index}_code"] = effect["codes"][observation]
                row[f"effect{effect_index}_intercept"] = int(effect["intercept"])
                slopes = effect.get("slopes", [])
                row[f"effect{effect_index}_slope0"] = slopes[0][observation] if slopes else ""
            for rhs_index, rhs in enumerate(fixture["rhs"]):
                row[f"rhs{rhs_index}_input"] = fixture["input_rhs"][rhs_index][observation]
                row[f"rhs{rhs_index}_fitted"] = rhs["fitted"][observation]
                row[f"rhs{rhs_index}_residual"] = rhs["residual"][observation]
                row[f"rhs{rhs_index}_objective"] = rhs["objective"]
            writer.writerow(row)
    return destination.getvalue()


def generate() -> dict[str, Any]:
    payload = {
        "schema": SCHEMA,
        "decimal_precision_digits": PRECISION,
        "rank_threshold": str(RANK_THRESHOLD),
        "accuracy_envelope": {"atol": 1e-9, "rtol": 1e-7},
        "fixtures": [solve_fixture(fixture) for fixture in fixtures()],
    }
    payload["flat_csv_sha256"] = hashlib.sha256(flat_csv(payload).encode("utf-8")).hexdigest()
    payload["checksum_sha256"] = _checksum(payload)
    return payload


def verify(sealed: dict[str, Any]) -> None:
    if sealed.get("schema") != SCHEMA:
        raise OracleError("sealed oracle schema mismatch")
    checksum = sealed.get("checksum_sha256")
    without_checksum = {key: value for key, value in sealed.items() if key != "checksum_sha256"}
    if checksum != _checksum(without_checksum):
        raise OracleError("sealed oracle checksum mismatch")
    regenerated = generate()
    if regenerated != sealed:
        raise OracleError("sealed oracle differs from the deterministic 100-digit regeneration")


def verify_flat_csv(sealed: dict[str, Any], text: str) -> None:
    expected = sealed.get("flat_csv_sha256")
    actual = hashlib.sha256(text.encode("utf-8")).hexdigest()
    if actual != expected:
        raise OracleError("sealed flat oracle CSV checksum mismatch")
    if text != flat_csv(sealed):
        raise OracleError("flat oracle CSV differs from the sealed Decimal outputs")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", type=Path, help="write the deterministic sealed JSON")
    parser.add_argument("--verify", type=Path, help="verify an existing sealed JSON")
    arguments = parser.parse_args()
    if bool(arguments.write) == bool(arguments.verify):
        parser.error("provide exactly one of --write or --verify")
    if arguments.write:
        payload = generate()
        arguments.write.parent.mkdir(parents=True, exist_ok=True)
        arguments.write.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        arguments.write.with_suffix(".csv").write_text(flat_csv(payload), encoding="utf-8")
    else:
        sealed = json.loads(arguments.verify.read_text(encoding="utf-8"))
        verify(sealed)
        verify_flat_csv(sealed, arguments.verify.with_suffix(".csv").read_text(encoding="utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
