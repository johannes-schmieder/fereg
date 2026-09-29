"""Dependency-free one-through-five-way PPML cluster covariance oracle.

This module is imported by the sealed qualification oracle driver.  It stays
separate from both fereg production code and the unittest module that checks
it, so the exact executed numerical dependency can be frozen in the release
manifest.
"""
from __future__ import annotations

import math
import sys
from collections.abc import Hashable, Sequence

from ppml_explicit_dummy_oracle import invert, matmul, outer


Matrix = list[list[float]]


def _repair_multiway_psd(matrix: Matrix) -> Matrix:
    """Apply the frozen post-inclusion Jacobi eigenvalue clipping policy."""
    dimension = len(matrix)
    work = [[(matrix[r][c] + matrix[c][r]) / 2.0 for c in range(dimension)] for r in range(dimension)]
    vectors = [[float(r == c) for c in range(dimension)] for r in range(dimension)]
    scale = max((sum(abs(value) for value in row) for row in work), default=0.0)
    scale = max(scale, sys.float_info.min)
    threshold = 128.0 * math.ulp(1.0) * scale
    for _ in range(dimension * dimension * 128):
        maximum, p, q = 0.0, 0, 1
        for row in range(dimension):
            for column in range(row + 1, dimension):
                candidate = abs(work[row][column])
                if candidate > maximum:
                    maximum, p, q = candidate, row, column
        if maximum <= threshold:
            break
        app, aqq, apq = work[p][p], work[q][q], work[p][q]
        tau = (aqq - app) / (2.0 * apq)
        tangent = math.copysign(1.0, tau) / (abs(tau) + math.sqrt(1.0 + tau * tau))
        cosine = 1.0 / math.sqrt(1.0 + tangent * tangent)
        sine = tangent * cosine
        for index in range(dimension):
            if index not in (p, q):
                aip, aiq = work[index][p], work[index][q]
                work[index][p] = work[p][index] = cosine * aip - sine * aiq
                work[index][q] = work[q][index] = sine * aip + cosine * aiq
        work[p][p], work[q][q] = app - tangent * apq, aqq + tangent * apq
        work[p][q] = work[q][p] = 0.0
        for index in range(dimension):
            vip, viq = vectors[index][p], vectors[index][q]
            vectors[index][p] = cosine * vip - sine * viq
            vectors[index][q] = sine * vip + cosine * viq
    else:
        raise ValueError("cluster oracle PSD eigensystem did not converge")
    eigenvalues = [max(0.0, work[index][index]) for index in range(dimension)]
    repaired = [[0.0] * dimension for _ in range(dimension)]
    for row in range(dimension):
        for column in range(row, dimension):
            value = sum(vectors[row][axis] * eigenvalues[axis] * vectors[column][axis] for axis in range(dimension))
            repaired[row][column] = repaired[column][row] = value
    return repaired


def _add_scaled(target: Matrix, source: Sequence[Sequence[float]], scale: float) -> None:
    for target_row, source_row in zip(target, source):
        for column, value in enumerate(source_row):
            target_row[column] += scale * value


def explicit_multiway_cluster_covariance(
    design: Sequence[Sequence[float]],
    outcome: Sequence[float],
    mean: Sequence[float],
    hessian: Sequence[Sequence[float]],
    prior_weights: Sequence[float],
    cluster_dimensions: Sequence[Sequence[Hashable]],
    *,
    correction: float = 1.0,
) -> tuple[Matrix, list[tuple[int, int]]]:
    """Return CGM inclusion--exclusion covariance and subset diagnostics."""

    rows = len(design)
    if (
        rows == 0
        or len(outcome) != rows
        or len(mean) != rows
        or len(prior_weights) != rows
        or not design[0]
        or any(len(row) != len(design[0]) for row in design)
    ):
        raise ValueError("cluster oracle input dimensions are invalid")
    dimensions = len(cluster_dimensions)
    if not 1 <= dimensions <= 5:
        raise ValueError("cluster oracle requires one through five dimensions")
    if any(len(axis) != rows for axis in cluster_dimensions):
        raise ValueError("cluster dimension and design lengths differ")
    if not math.isfinite(correction) or correction <= 0.0:
        raise ValueError("cluster correction must be finite and positive")
    if any(not math.isfinite(float(weight)) or float(weight) <= 0.0 for weight in prior_weights):
        raise ValueError("cluster prior weights must be finite and positive")

    columns = len(design[0])
    scores = [
        [float(weight) * (float(y_value) - float(mu_value)) * float(value) for value in row]
        for row, y_value, mu_value, weight in zip(
            design, outcome, mean, prior_weights
        )
    ]
    meat = [[0.0] * columns for _ in range(columns)]
    diagnostics: list[tuple[int, int]] = []
    for mask in range(1, 1 << dimensions):
        axes = [axis for axis in range(dimensions) if mask & (1 << axis)]
        grouped: dict[tuple[Hashable, ...], list[float]] = {}
        for row, score in enumerate(scores):
            key = tuple(cluster_dimensions[axis][row] for axis in axes)
            aggregate = grouped.setdefault(key, [0.0] * columns)
            for column, value in enumerate(score):
                aggregate[column] += value
        subset = [[0.0] * columns for _ in range(columns)]
        for aggregate in grouped.values():
            _add_scaled(subset, outer(aggregate, aggregate), 1.0)
        _add_scaled(meat, subset, 1.0 if len(axes) % 2 else -1.0)
        diagnostics.append((mask, len(grouped)))
    bread = invert(hessian)
    covariance = [[correction * value for value in row] for row in matmul(matmul(bread, meat), bread)]
    return (
        covariance,
        diagnostics,
    )
