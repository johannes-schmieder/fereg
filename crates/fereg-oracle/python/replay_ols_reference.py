#!/usr/bin/env python3
"""Independent Decimal witnesses for the two explicit-LSMR replay fixtures.

Read binary64 inputs exactly, partial out an explicit dummy design by dense
weighted elimination, and derive adjusted R-squared and the two-column rank
condition proxy algebraically. No plugin result enters the reference.
"""
from __future__ import annotations

from collections import Counter
import copy
from decimal import Decimal, localcontext
import math
from pathlib import Path

from replay_bias_reference import digest, read_case

NAMES = ('ols-10', 'ols-13')
MISSING = float.fromhex('0x1p+1023')


def _solve(matrix, rhs):
    """Small full-rank system, using Decimal row-pivoted elimination."""
    rows = [list(row) + [value] for row, value in zip(matrix, rhs)]
    for column in range(len(rows)):
        pivot = max(range(column, len(rows)), key=lambda i: abs(rows[i][column]))
        rows[column], rows[pivot] = rows[pivot], rows[column]
        divisor = rows[column][column]
        if divisor == 0:
            raise ValueError('OLS replay dummy design is not identified')
        rows[column] = [value / divisor for value in rows[column]]
        for i in range(len(rows)):
            if i != column:
                factor = rows[i][column]
                rows[i] = [a - factor * b for a, b in zip(rows[i], rows[column])]
    return [row[-1] for row in rows]


def dense_targets(outcome, regressors, effects, weights, frequency):
    """Two finite columns; one anchor in each FE after the first."""
    if (len(regressors) != 2 or not effects or not outcome or
            any(len(column) != len(outcome) for column in [*regressors, *effects, weights])):
        raise ValueError('OLS replay reference requires two columns and aligned rows')
    with localcontext() as context:
        context.prec = 70
        y = [Decimal.from_float(value) for value in outcome]
        x = [[Decimal.from_float(value) for value in column] for column in regressors]
        w = [Decimal.from_float(value) for value in weights]
        dummy = []
        for effect_index, effect in enumerate(effects):
            levels = sorted(set(effect))[int(effect_index > 0):]
            dummy.extend([[Decimal(int(value == level)) for value in effect] for level in levels])

        def dot(left, right):
            return sum((weight * a * b for weight, a, b in zip(w, left, right)), Decimal(0))

        gram = [[dot(a, b) for b in dummy] for a in dummy]

        def within(column):
            coefficients = _solve(gram, [dot(a, column) for a in dummy])
            return [value - sum((a[i] * b for a, b in zip(dummy, coefficients)), Decimal(0))
                    for i, value in enumerate(column)]

        projected = [within(column) for column in x]
        response = within(y)
        finite_gram = [[dot(a, b) for b in projected] for a in projected]
        coefficients = _solve(finite_gram, [dot(a, response) for a in projected])
        residual = [value - sum((a[i] * b for a, b in zip(projected, coefficients)), Decimal(0))
                    for i, value in enumerate(response)]
        effective_n = sum(w) if frequency else Decimal(len(y))
        residual_dof = effective_n - len(dummy) - len(x)
        if residual_dof <= 0:
            raise ValueError('OLS replay reference lacks residual degrees of freedom')
        mean = sum((weight * value for weight, value in zip(w, y)), Decimal(0)) / sum(w)
        centered = [value - mean for value in y]
        adjusted_r2 = 1 - ((dot(residual, residual) / residual_dof) /
                           (dot(centered, centered) / (effective_n - 1)))

        # The finite solve scales by ORIGINAL weighted column norms. For two
        # columns, the pivoted triangular diagonals follow from the 2x2 Gram.
        norms = [dot(column, column).sqrt() for column in x]
        a = finite_gram[0][0] / norms[0] ** 2
        c = finite_gram[1][1] / norms[1] ** 2
        b = finite_gram[0][1] / (norms[0] * norms[1])
        leading, trailing = max(a, c), min(a, c)
        condition_proxy = (leading / (trailing - b * b / leading)).sqrt()
        return {'RFIT': {'11': float(adjusted_r2)}, 'RCERT': {'11': float(condition_proxy)}}


def derive_ols(path: Path):
    args, data, matrices = read_case(path, expected_action='estimate-ols')
    effects = 2 if path.stem == 'ols-10' else 3
    frequency = effects == 2
    sources = effects + 2
    input_columns = sources + 3
    bindings = [[1, 1, 0], [2, 2, 0]] + [[i + 2, 3, i] for i in range(1, sources + 1)]
    bindings.append([input_columns, 5 if frequency else 4, 0])
    metadata = {
        'S': [[i, 0] for i in range(1, sources + 1)],
        'T': [[i, i, i, 1, 1, 0, 0, 0] for i in (1, 2)],
        'C': [[i, 1, i, 1, 1, 0, 0, 2, 1, i, i, 0, 0] for i in (1, 2)],
        'A': [[i, 1, 1, 0] for i in range(1, effects + 1)],
        'AC': [[i, 1, 1, i + 2] for i in range(1, effects + 1)],
        'BIND': bindings, 'CT': [[0, 0]], 'CC': [[0, 0, 0]],
    }
    fields = {'model_major': '2', 'model_minor': '1',
              'source_count': str(sources), 'term_count': '2', 'component_count': '2',
              'absorb_term_count': str(effects), 'absorb_component_count': str(effects),
              'binding_count': str(input_columns), 'cluster_component_count': '0',
              'include_intercept': '1', 'keep_singletons': '0', 'solver': 'lsmr',
              'dof': 'pairwise', 'cluster_term_count': '0',
              'vce': 'iid' if frequency else 'robust', 'memory': 'fast' if frequency else 'low'}
    table_names = {'source': 'S', 'term': 'T', 'component': 'C', 'absorb_term': 'A',
                   'absorb_component': 'AC', 'binding': 'BIND',
                   'cluster_term': 'CT', 'cluster_component': 'CC'}
    fields.update({key + '_matrix': value for key, value in table_names.items()})
    if (path.stem not in NAMES or len(data) != input_columns + 6 or len(data[0]) != 150 or
            any(matrices.get(key) != value for key, value in metadata.items()) or
            any(args.get(key) != value for key, value in fields.items()) or
            data[0] != [1.] * 150):
        raise ValueError('OLS replay model differs from the declared two-column fixtures')
    required = data[1:input_columns]
    if any(not math.isfinite(value) for column in required for value in column):
        raise ValueError('OLS replay contains a nonfinite numeric input')
    rows = [i for i in range(150) if all(column[i] != MISSING for column in required)]
    weights = data[input_columns - 1]
    if any(weights[i] <= 0 or (frequency and weights[i] != int(weights[i])) for i in rows):
        raise ValueError('OLS replay weights are outside the fixture domain')
    for column in data[4:4 + effects]:
        if any(column[i] < 0 or column[i] != int(column[i]) for i in rows):
            raise ValueError('OLS replay FE levels are outside the fixture domain')
    while True:
        previous = len(rows)
        for column in data[4:4 + effects]:
            counts = Counter()
            for i in rows:
                counts[column[i]] += int(weights[i]) if frequency else 1
            rows = [i for i in rows if counts[column[i]] > 1]
        if len(rows) == previous:
            break
    if [len({column[i] for i in rows}) for column in data[4:4 + effects]] != [6, 5, 4][:effects]:
        raise ValueError('OLS replay retained topology differs from the declared fixtures')
    take = lambda column: [column[i] for i in rows]
    targets = dense_targets(take(data[1]), [take(column) for column in data[2:4]],
                            [take(column) for column in data[4:4 + effects]],
                            take(weights), frequency)
    return rows, targets


def enrich_expected(path: Path, expected: dict) -> dict:
    if path.stem in {'ols-08', 'ols-11', 'ols-14', 'ols-26', 'ols-29'}:
        from replay_cmg_reference import enrich_expected as enrich_cmg_expected
        return enrich_cmg_expected(path, expected)
    if path.stem not in NAMES:
        return expected
    rows, targets = derive_ols(path)
    if (expected.get('poisson') or rows != expected['rows'] or
            expected['sample'] != [int(i in rows) for i in range(150)]):
        raise ValueError('OLS replay sample disagrees with independent preparation')
    output = copy.deepcopy(expected)
    output['ols_reference'] = {'input_sha256': digest(path), 'targets': targets}
    return output
