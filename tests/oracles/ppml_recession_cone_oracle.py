"""Dependency-free exact-rational PPML recession-cone oracle.

This module deliberately knows nothing about fereg's Rust implementation.
For each observation it asks whether the homogeneous model span contains a
direction that is zero on positive outcomes, nonpositive on every zero
outcome, and strictly negative on that observation.  Exact rational
Fourier--Motzkin elimination supplies a witness when the answer is yes.
"""
from __future__ import annotations

from fractions import Fraction
from typing import Iterable, Sequence


class ConeOracleError(ValueError):
    """The bounded oracle input is malformed."""


def rational(value: object) -> Fraction:
    if isinstance(value, bool):
        raise ConeOracleError("boolean is not an oracle number")
    if isinstance(value, Fraction):
        return value
    if isinstance(value, int):
        return Fraction(value)
    if isinstance(value, str):
        try:
            return Fraction(value)
        except (ValueError, ZeroDivisionError) as error:
            raise ConeOracleError(f"invalid rational value: {value!r}") from error
    raise ConeOracleError("oracle numbers must be integers or rational strings")


Constraint = tuple[tuple[Fraction, ...], Fraction]


def _solve(constraints: Sequence[Constraint], variables: int) -> tuple[Fraction, ...] | None:
    """Solve ``a*x <= b`` exactly using bounded Fourier--Motzkin elimination."""
    if variables == 0:
        return () if all(bound >= 0 for coefficients, bound in constraints if not coefficients) else None
    positive: list[Constraint] = []
    negative: list[Constraint] = []
    zero: list[Constraint] = []
    for coefficients, bound in constraints:
        coefficient = coefficients[-1]
        record = (coefficients[:-1], bound)
        if coefficient > 0:
            positive.append((record[0] + (coefficient,), bound))
        elif coefficient < 0:
            negative.append((record[0] + (coefficient,), bound))
        else:
            zero.append(record)
    reduced: list[Constraint] = list(zero)
    for upper_coefficients, upper_bound in positive:
        upper_scale = upper_coefficients[-1]
        for lower_coefficients, lower_bound in negative:
            lower_scale = -lower_coefficients[-1]
            coefficients = tuple(
                upper_coefficients[index] / upper_scale
                + lower_coefficients[index] / lower_scale
                for index in range(variables - 1)
            )
            reduced.append(
                (coefficients, upper_bound / upper_scale + lower_bound / lower_scale)
            )
    prefix = _solve(reduced, variables - 1)
    if prefix is None:
        return None
    lower_bounds: list[Fraction] = []
    upper_bounds: list[Fraction] = []
    for coefficients, bound in constraints:
        rest = sum(
            (coefficient * value for coefficient, value in zip(coefficients[:-1], prefix)),
            Fraction(0),
        )
        coefficient = coefficients[-1]
        if coefficient > 0:
            upper_bounds.append((bound - rest) / coefficient)
        elif coefficient < 0:
            lower_bounds.append((bound - rest) / coefficient)
        elif rest > bound:
            return None
    lower = max(lower_bounds) if lower_bounds else None
    upper = min(upper_bounds) if upper_bounds else None
    if lower is not None and upper is not None and lower > upper:
        return None
    if lower is not None:
        value = max(lower, Fraction(0)) if upper is None else lower
    elif upper is not None:
        value = min(upper, Fraction(0))
    else:
        value = Fraction(0)
    witness = prefix + (value,)
    if any(sum((a * x for a, x in zip(row, witness)), Fraction(0)) > bound for row, bound in constraints):
        raise AssertionError("Fourier--Motzkin witness reconstruction failed")
    return witness


def recession_witness(
    design: Sequence[Sequence[object]], outcome: Sequence[object], target: int
) -> tuple[Fraction, ...] | None:
    """Return an exact direction certifying ``target``, or ``None``."""
    rows = [tuple(rational(value) for value in row) for row in design]
    outcomes = [rational(value) for value in outcome]
    if not rows or len(rows) != len(outcomes):
        raise ConeOracleError("design/outcome row counts are invalid")
    columns = len(rows[0])
    if columns == 0 or any(len(row) != columns for row in rows):
        raise ConeOracleError("design must be a nonempty rectangular matrix")
    if not 0 <= target < len(rows) or outcomes[target] != 0:
        raise ConeOracleError("target must identify a zero-outcome row")
    if any(value < 0 for value in outcomes):
        raise ConeOracleError("PPML outcomes must be nonnegative")
    constraints: list[Constraint] = []
    zero = Fraction(0)
    for row, value in zip(rows, outcomes):
        if value > 0:
            constraints.append((row, zero))
            constraints.append((tuple(-item for item in row), zero))
        else:
            constraints.append((row, zero))
    constraints.append((rows[target], Fraction(-1)))
    return _solve(constraints, columns)


def separated_rows(
    design: Sequence[Sequence[object]], outcome: Sequence[object]
) -> tuple[tuple[int, ...], dict[int, tuple[str, ...]]]:
    """Return every certifiably separated row and one exact witness per row."""
    rows: list[int] = []
    witnesses: dict[int, tuple[str, ...]] = {}
    for index, value in enumerate(outcome):
        if rational(value) != 0:
            continue
        witness = recession_witness(design, outcome, index)
        if witness is not None:
            rows.append(index)
            witnesses[index] = tuple(str(value) for value in witness)
    return tuple(rows), witnesses


def evaluate_direction(
    design: Sequence[Sequence[object]], direction: Iterable[object]
) -> tuple[Fraction, ...]:
    values = tuple(rational(value) for value in direction)
    return tuple(
        sum((rational(coefficient) * value for coefficient, value in zip(row, values)), Fraction(0))
        for row in design
    )
