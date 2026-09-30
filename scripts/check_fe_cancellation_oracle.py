#!/usr/bin/env python3
"""Regenerate the captured FE-cancellation witness at 100 decimal digits.

Inputs are exact binary64 values, converted with Decimal.from_float. The
existing independent explicit-design oracle selects its own basis and solves
at 100 digits. No production numerical implementation supplies the witness.
"""
from __future__ import annotations
import csv
from decimal import Decimal
from pathlib import Path
from weak_graph_decimal_oracle import solve_fixture

REFERENCE = Path(__file__).resolve().parents[1] / "tests/reference/fe_cancellation_decimal_oracle_v1.csv"


def verify() -> None:
    with REFERENCE.open(newline="") as source:
        rows = list(csv.DictReader(source))
    assert len(rows) == 206
    assert [int(r["row"]) for r in rows] == list(range(len(rows)))
    codes = [[int(r[f"effect{i}"]) for r in rows] for i in range(3)]
    fixture = {
        "id": "mu-1047-cancelled-fe-response", "tags": [],
        "weights": [Decimal.from_float(float(r["weight"])) for r in rows],
        "effects": [{"codes": c, "levels": max(c) + 1, "intercept": True, "slopes": []} for c in codes],
        "rhs": [[Decimal.from_float(float(r["response"])) for r in rows]],
    }
    witness = solve_fixture(fixture)
    assert witness["rank"] == 12
    assert witness["rhs"][0]["residual"] == [r["residual"] for r in rows]
    print("FE cancellation: exact binary64 inputs, 100-digit residual witness verified")


if __name__ == "__main__":
    verify()
