#!/usr/bin/env python3
"""Verify the synthetic seed-155 boundary response with exact rational arithmetic.

The Rust oracle test independently checks every captured input against its
generator. Binary64 values are exact rationals here; 100-digit explicit-design
residuals from the existing decimal oracle are checked against the exact solve.
No production numerical implementation supplies the witness.
"""
from __future__ import annotations

import argparse
import csv
from decimal import Decimal, localcontext
from fractions import Fraction
from pathlib import Path

import weak_graph_decimal_oracle as decimal_oracle

REFERENCE = Path(__file__).resolve().parents[1] / "tests/reference/fe_boundary_response_decimal_oracle_v1.csv"
FIELDS = ["row", "effect0", "slope0", "effect1", "slope1", "effect2", "slope2", "weight", "response", "raw0", "raw1", "raw2", "residual"]


def captured_inputs() -> list[dict[str, str]]:
    state = 155
    mask = (1 << 64) - 1

    def next_u64() -> int:
        nonlocal state
        state = (state + 0x9E3779B97F4A7C15) & mask
        z = ((state ^ (state >> 30)) * 0xBF58476D1CE4E5B9) & mask
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & mask
        return z ^ (z >> 31)

    def continuous() -> float:
        return 2.0 * ((next_u64() >> 11) * (1.0 / (1 << 53))) - 1.0

    n = 79
    # Keep each move immediately after its worker's sixteen home observations.
    edges = []
    for a in range(6):
        edges.extend([(a, a % 5)] * 16)
        edges.append((a, (a + 1) % 5))
    codes = [[edge[j] for edge in edges[:n]] for j in range(2)]
    codes.append([next_u64() % 3 for _ in range(n)])
    slopes = [[continuous() for _ in range(n)] for _ in range(3)]
    raw = [[continuous() for _ in range(n)] for _ in range(2)]
    response = [float(row % 7 == 1) for row in range(n)]
    boundary = sum(response)
    weights = [(1e-5 * 1e-5) / boundary if u else 1.0 for u in response]
    ray = [-u for u in response]
    for effect in range(3):
        for row in range(n):
            alpha = (effect + 1 + codes[effect][row]) * 0.0625
            ray[row] -= alpha
            ray[row] -= alpha * slopes[effect][row]
    raw.append(ray)
    rows = []
    for row in range(n):
        values = {"row": str(row), "weight": format(weights[row], ".17e"), "response": format(response[row], ".17e")}
        for effect in range(3):
            values[f"effect{effect}"] = str(codes[effect][row])
            values[f"slope{effect}"] = format(slopes[effect][row], ".17e")
            values[f"raw{effect}"] = format(raw[effect][row], ".17e")
        rows.append(values)
    return rows


def fixture(rows: list[dict[str, str]]) -> dict:
    return {
        "id": "seed-155-boundary-response", "tags": [],
        "weights": [Decimal.from_float(float(r["weight"])) for r in rows],
        "rhs": [[Decimal.from_float(float(r["response"])) for r in rows]],
        "effects": [{
            "codes": [int(r[f"effect{i}"]) for r in rows],
            "levels": 1 + max(int(r[f"effect{i}"]) for r in rows),
            "intercept": True,
            "slopes": [[Decimal.from_float(float(r[f"slope{i}"])) for r in rows]],
        } for i in range(3)],
    }


def exact_residual(rows: list[dict[str, str]]) -> tuple[int, list[Fraction]]:
    weights = [Fraction(float(r["weight"])) for r in rows]
    response = [Fraction(float(r["response"])) for r in rows]
    columns = []
    for effect in range(3):
        for level in range(1 + max(int(r[f"effect{effect}"]) for r in rows)):
            for slope in [False, True]:
                columns.append([
                    (Fraction(float(r[f"slope{effect}"])) if slope else Fraction(1))
                    if int(r[f"effect{effect}"]) == level else Fraction(0) for r in rows
                ])
    m = len(columns)
    gram = [[sum((w * a * b for w, a, b in zip(weights, left, right)), Fraction(0)) for right in columns] for left in columns]
    moments = [sum((w * a * b for w, a, b in zip(weights, column, response)), Fraction(0)) for column in columns]
    selected = []
    # Exact PSD elimination can skip only an exactly zero remaining direction.
    for pivot in range(m):
        if gram[pivot][pivot] == 0:
            assert all(v == 0 for v in gram[pivot][pivot:]) and moments[pivot] == 0
            continue
        assert gram[pivot][pivot] > 0
        selected.append(pivot)
        for row in range(pivot + 1, m):
            multiplier = gram[row][pivot] / gram[pivot][pivot]
            for column in range(pivot + 1, m):
                gram[row][column] -= multiplier * gram[pivot][column]
            moments[row] -= multiplier * moments[pivot]
            gram[row][pivot] = Fraction(0)
    coefficients = [Fraction(0)] * m
    for row in reversed(selected):
        coefficients[row] = (moments[row] - sum((gram[row][column] * coefficients[column] for column in range(row + 1, m)), Fraction(0))) / gram[row][row]
    residual = [u - sum((column[row] * alpha for column, alpha in zip(columns, coefficients)), Fraction(0)) for row, u in enumerate(response)]
    assert all(sum((w * x * r for w, x, r in zip(weights, column, residual)), Fraction(0)) == 0 for column in columns)
    return len(selected), residual


def verify(write: bool = False) -> None:
    expected = captured_inputs()
    inputs = fixture(expected)
    witness = decimal_oracle.solve_fixture(inputs)
    rank, rational = exact_residual(expected)
    assert rank == witness["rank"] == 24
    assert rational[50] == 0
    with localcontext() as context:
        context.prec = decimal_oracle.PRECISION
        columns, _ = decimal_oracle._explicit_columns(inputs)
        selected, _ = decimal_oracle._independent_columns(columns, inputs["weights"])
        fitted = decimal_oracle._projection([columns[i] for i in selected], inputs["weights"], inputs["rhs"][0])
        unrounded = [u - fit for u, fit in zip(inputs["rhs"][0], fitted)]
        # The existing oracle writes 70 digits after the leading digit.
        for value, text in zip(unrounded, witness["rhs"][0]["residual"]):
            serialized = Decimal(text)
            quantum = Decimal(1).scaleb(serialized.adjusted() - 70)
            assert abs(serialized - value) <= quantum / 2
    with localcontext() as context:
        context.prec = 120
        error = max(abs(Decimal(value.numerator) / Decimal(value.denominator) - approximate) for value, approximate in zip(rational, unrounded))
        assert error < Decimal("1e-85"), error
    for row, residual in zip(expected, witness["rhs"][0]["residual"]):
        row["residual"] = residual
    if write:
        with REFERENCE.open("w", newline="") as target:
            writer = csv.DictWriter(target, fieldnames=FIELDS, lineterminator="\n")
            writer.writeheader()
            writer.writerows(expected)
    with REFERENCE.open(newline="") as source:
        assert list(csv.DictReader(source)) == expected
    print(f"Boundary response: 79 exact binary64 rows, rank {rank}, exact row-50 residual zero; decimal discrepancy {error:.3e}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    verify(parser.parse_args().write)
