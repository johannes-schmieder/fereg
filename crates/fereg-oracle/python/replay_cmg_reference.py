"""Independent 70-digit full-dummy witnesses for five explicit-CMG OLS cases.

The reported constant is a linear functional of an identified full dummy fit.
Transform the full inverse and score covariance by that functional; no production
projection, preconditioner, normalization, or covariance routine is called.
"""
from __future__ import annotations

import copy
from decimal import Decimal, localcontext
import math

from replay_bias_reference import read_case
from replay_ols_reference import _solve, dense_targets
from replay_psd import eigensystem

CASES = {'ols-08': (2, 'unit', 'iid', 'fast'),
         'ols-11': (2, 'probability', 'cluster', 'fast'),
         'ols-14': (3, 'frequency', 'iid', 'low'),
         'ols-26': (2, 'frequency', 'iid', 'low'),
         'ols-29': (3, 'analytic', 'robust', 'low')}


def normalized_targets(outcome, regressors, effects, weights, *, frequency=False, vce='iid', clusters=None):
    """Full-rank connected categorical witness, with one anchor after FE1."""
    n, k = len(outcome), len(regressors)
    if (not n or not k or not effects or any(len(c) != n for c in [*regressors, *effects, weights])
            or any(not math.isfinite(v) for c in [outcome, *regressors, weights] for v in c)
            or any(w <= 0 or (frequency and w != int(w)) for w in weights)
            or vce not in {'iid', 'robust', 'cluster'}
            or (vce == 'cluster' and (clusters is None or len(clusters) != n))):
        raise ValueError('invalid full-dummy normalized-intercept witness')
    with localcontext() as context:
        context.prec = 70
        dec = Decimal.from_float
        y = [dec(float(v)) for v in outcome]
        columns = [[dec(float(v)) for v in c] for c in regressors]
        w = [dec(float(v)) for v in weights]
        levels = [sorted(set(effect)) for effect in effects]
        positions = []
        for effect_index, (effect, categories) in enumerate(zip(effects, levels)):
            positions.append({})
            for level in categories[int(effect_index > 0):]:
                positions[-1][level] = len(columns)
                columns.append([Decimal(int(value == level)) for value in effect])
        rank = len(columns)
        total_weight = sum(w)
        effective_n = total_weight if frequency else Decimal(n)
        df = effective_n - rank
        if df <= 0:
            raise ValueError('full-dummy reference has no residual degrees of freedom')
        dot = lambda a, b: sum((weight * x * z for weight, x, z in zip(w, a, b)), Decimal(0))
        gram = [[dot(a, b) for b in columns] for a in columns]
        coefficients = _solve(gram, [dot(c, y) for c in columns])
        # Direct Gauss-Jordan inverses at 70 digits are independent of the
        # runtime's sparse graph solves and finite QR/inference implementation.
        inverse_columns = [_solve(gram, [Decimal(int(i == j)) for i in range(rank)]) for j in range(rank)]
        inverse = [[inverse_columns[j][i] for j in range(rank)] for i in range(rank)]
        fitted = [sum((c[i] * b for c, b in zip(columns, coefficients)), Decimal(0)) for i in range(n)]
        residual = [a - b for a, b in zip(y, fitted)]
        transform = [[Decimal(int(i == j)) for j in range(rank)] for i in range(k)]
        # The global reported constant is the weighted mean of the dummy fit.
        transform.append([Decimal(0)] * k + [sum(weight * value for weight, value in zip(w, c)) / total_weight
                                             for c in columns[k:]])
        beta = [sum(t * b for t, b in zip(row, coefficients)) for row in transform]
        tb = [[sum(transform[i][a] * inverse[a][b] for a in range(rank)) for b in range(rank)] for i in range(k + 1)]
        influence = [[sum(tb[j][a] * columns[a][i] for a in range(rank)) for i in range(n)] for j in range(k + 1)]
        if vce == 'iid':
            sigma2 = dot(residual, residual) / df
            covariance = [[sigma2 * sum(tb[i][a] * transform[j][a] for a in range(rank))
                           for j in range(k + 1)] for i in range(k + 1)]
        elif vce == 'robust':
            multipliers = [weight if frequency else weight * weight for weight in w]
            covariance = [[effective_n / df * sum(multiplier * error * error * influence[i][row] * influence[j][row]
                           for row, (multiplier, error) in enumerate(zip(multipliers, residual)))
                           for j in range(k + 1)] for i in range(k + 1)]
        else:
            groups = sorted(set(clusters))
            if len(groups) <= 1:
                raise ValueError('cluster reference requires at least two groups')
            scores = [[sum(w[row] * residual[row] * influence[j][row] for row in range(n) if clusters[row] == group)
                       for group in groups] for j in range(k + 1)]
            correction = Decimal(len(groups)) / (len(groups) - 1) * (effective_n - 1) / df
            covariance = [[correction * sum(a * b for a, b in zip(scores[i], scores[j]))
                           for j in range(k + 1)] for i in range(k + 1)]
        channels = []
        for effect, lookup in zip(effects, positions):
            channel = [coefficients[lookup[level]] if level in lookup else Decimal(0) for level in effect]
            mean = sum(weight * value for weight, value in zip(w, channel)) / total_weight
            channels.append([float(value - mean) for value in channel])
        dummy = columns[k:]
        dummy_fit = _solve([row[k:] for row in gram[k:]], [dot(c, y) for c in dummy])
        within_y = [y[i] - sum(c[i] * b for c, b in zip(dummy, dummy_fit)) for i in range(n)]
        reference_scale = Decimal(1) if frequency else Decimal(n) / total_weight
        rss = dot(residual, residual) * reference_scale
        mean_y = sum(weight * value for weight, value in zip(w, y)) / total_weight
        tss = dot([v - mean_y for v in y], [v - mean_y for v in y]) * reference_scale
        within_tss = dot(within_y, within_y) * reference_scale
        finite_cov = [row[:k] for row in covariance[:k]]
        wald_solution = _solve(finite_cov, beta[:k])
        wald = sum(a * b for a, b in zip(beta[:k], wald_solution)) / k
        likelihood = lambda ss: -float(effective_n) / 2 * (math.log(2 * math.pi) + 1 + math.log(float(ss / effective_n)))
        fit_targets = [rss, tss, within_tss, tss - rss, 1 - rss / tss,
                       1 - (rss / df) / (tss / (effective_n - 1)),
                       1 - rss / within_tss, 1 - (rss / df) / (within_tss / (effective_n - rank + k)),
                       (rss / df).sqrt(), wald, likelihood(rss), likelihood(within_tss)]
        reported_bread = [[sum(tb[i][a] * transform[j][a] for a in range(rank)) / reference_scale
                           for j in range(k + 1)] for i in range(k + 1)]
        if k != 2:
            raise ValueError('CMG witness requires exactly two finite regressors')
        a, b, c = covariance
        determinant = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
        certificate_targets = {'27': float(min(reported_bread[i][i] for i in range(k + 1))), '61': float(determinant)}
        if vce == 'iid':
            certificate_targets['41'] = float(min(covariance[i][i] for i in range(k + 1)))
        if vce == 'cluster':
            certificate_targets['55'] = min(eigensystem([[float(v) for v in row] for row in covariance])[0])
        xb = [beta[-1] + sum(columns[j][i] * beta[j] for j in range(k)) for i in range(n)]
        return {'beta': list(map(float, beta)), 'covariance': [list(map(float, row)) for row in covariance],
                'residual': list(map(float, residual)), 'fitted': list(map(float, fitted)),
                'saved_channels': channels, 'rank': rank, 'fe_rank': rank - k, 'precision_digits': 70,
                'xb': list(map(float, xb)), 'absorbed': [float(f - x) for f, x in zip(fitted, xb)],
                'targets': {'RFIT': {str(2 * i + 1): float(v) for i, v in enumerate(fit_targets)},
                            'RCERT': certificate_targets}}



def derive(path):
    effect_count, weight_kind, vce, memory = CASES[path.stem]
    args, data, matrices = read_case(path, expected_action='estimate-ols')
    clustered = vce == 'cluster'
    source_count = 2 + effect_count + int(clustered)
    weight_role = {'unit': None, 'analytic': 4, 'frequency': 5, 'probability': 6}[weight_kind]
    binding = [[1, 1, 0], [2, 2, 0]] + [[i + 2, 3, i] for i in range(1, 3 + effect_count)]
    if weight_role is not None:
        binding.append([len(binding) + 1, weight_role, 0])
    if clustered:
        binding.append([len(binding) + 1, 3, source_count])
    expected_matrices = {
        'S': [[i, 0] for i in range(1, source_count + 1)],
        'T': [[i, i, i, 1, 1, 0, 0, 0] for i in (1, 2)],
        'C': [[i, 1, i, 1, 1, 0, 0, 2, 1, i, i, 0, 0] for i in (1, 2)],
        'A': [[i, 1, 1, 0] for i in range(1, effect_count + 1)],
        'AC': [[i, 1, 1, i + 2] for i in range(1, effect_count + 1)],
        'BIND': binding, 'CT': [[1, 1]] if clustered else [[0, 0]],
        'CC': [[1, 1, source_count]] if clustered else [[0, 0, 0]],
    }
    fields = {'solver': 'cmg', 'memory': memory, 'vce': vce, 'include_intercept': '1',
              'keep_singletons': '0', 'dof': 'pairwise', 'source_count': str(source_count),
              'term_count': '2', 'component_count': '2', 'absorb_term_count': str(effect_count),
              'absorb_component_count': str(effect_count), 'cluster_term_count': str(int(clustered)),
              'cluster_component_count': str(int(clustered)), 'binding_count': str(len(binding))}
    if (any(args.get(key) != value for key, value in fields.items())
            or any(matrices.get(key) != value for key, value in expected_matrices.items())
            or len(data[0]) != 150 or data[0] != [1.] * 150):
        raise ValueError('CMG reference model differs from the five declared numeric fixtures')
    effects = data[4:4 + effect_count]
    if [len(set(c)) for c in effects] != [6, 5, 4][:effect_count]:
        raise ValueError('CMG reference retained categorical levels differ')
    if any(not math.isfinite(v) or v < 0 or v != int(v) for c in effects for v in c):
        raise ValueError('CMG reference effect labels are invalid')
    weights = [1.] * 150 if weight_role is None else data[4 + effect_count]
    clusters = data[len(binding) - 1] if clustered else None
    target = normalized_targets(data[1], data[2:4], effects, weights,
                                frequency=weight_kind == 'frequency', vce=vce, clusters=clusters)
    target['targets']['RCERT']['11'] = dense_targets(data[1], data[2:4], effects, weights,
                                                      weight_kind == 'frequency')['RCERT']['11']
    tolerance = float(args['tolerance'])
    if tolerance != 1e-10:
        raise ValueError('CMG witness requested tolerance changed')
    # These metrics report numerical roundoff, not mathematical point targets.
    # Their bounds are fixed by the frozen request and independently checked
    # full-model residual moments; acceptance flags remain exact replay fields.
    residual_indices = [9, 13, 15, 17, 19, 21, 23, 25, 29, 31, 33, 35, 37, 39, 43, 45]
    if vce == 'cluster': residual_indices.append(41)
    bounds = {'RCERT': {str(i): [0., tolerance] for i in residual_indices}}
    if vce == 'robust': bounds['RCERT']['41'] = [0., 1. + tolerance]
    absolute_bound = tolerance * max(1., sum(weights) * max(abs(v) for c in data[1:4] for v in c))
    bounds['RSOLVER'] = {str(row * 16 + i): [0., absolute_bound if i == 4 else tolerance]
                         for row in range(2) for i in (4, 5, 6, 7)}
    target.update(bounds=bounds, tolerance=tolerance,
                  moment_inputs={'weights': weights, 'regressors': data[2:4], 'effects': effects})
    return target


def enrich_expected(path, expected):
    target = derive(path)
    if (expected.get('poisson') or expected.get('rows') != list(range(150))
            or expected.get('sample') != [1] * 150 or len(expected.get('beta', [])) != 2):
        raise ValueError('CMG independent target/sample layout differs')
    # Pinned finite-block and row targets remain an independent cross-check;
    # enrich only after the second, high-precision construction agrees.
    comparisons = [(target['beta'][:2], expected['beta'], 1e-7, 1e-10),
                   ([v for row in target['covariance'][:2] for v in row[:2]],
                    [v for row in expected['covariance'] for v in row], 1e-6, 1e-10),
                   (target['residual'], expected['residual'], 1e-7, 1e-9),
                   (target['fitted'], expected['fitted'], 1e-7, 1e-9)]
    for left, right, rtol, atol in comparisons:
        if len(left) != len(right) or any(abs(a - b) > atol + rtol * max(abs(a), abs(b)) for a, b in zip(left, right)):
            raise ValueError('70-digit CMG reference disagrees with pinned independent dense witness')
    enriched = copy.deepcopy(expected)
    enriched['beta'], enriched['covariance'] = target['beta'], target['covariance']
    saved = expected.get('saved', [])
    if saved and len(saved) != len(target['saved_channels']):
        raise ValueError('CMG saved reference channel count differs')
    enriched['saved_reference'] = dict(zip(saved, target['saved_channels']))
    enriched['column_reference'] = {expected['outputs'][key]: target[key] for key in ('xb', 'absorbed')}
    enriched['cmg_diagnostics'] = {key: target[key] for key in ('targets', 'bounds', 'tolerance', 'moment_inputs')}
    enriched['cmg_reference'] = {key: target[key] for key in ('precision_digits', 'rank', 'fe_rank')}
    return enriched


def diagnostic_errors(result, expected):
    """Validate independently derived values and original-design moment bounds."""
    import struct
    number = lambda value: struct.unpack('>d', bytes.fromhex(value))[0]
    diagnostics = expected['cmg_diagnostics']
    errors = []
    for name, targets in diagnostics['targets'].items():
        actual = [number(value) for value in result['matrices'][name]['bits']]
        for index, target in targets.items():
            i = int(index)
            if (actual[i - 1] != 1 or not math.isfinite(actual[i])
                    or abs(actual[i] - target) > 1e-14 + 1e-12 * max(abs(actual[i]), abs(target))):
                errors.append(f'independent CMG {name}/{i} target mismatch')
    for name, limits in diagnostics['bounds'].items():
        actual = [number(value) for value in result['matrices'][name]['bits']]
        for index, (lower, upper) in limits.items():
            i = int(index)
            if ((name == 'RCERT' and actual[i - 1] != 1)
                    or not math.isfinite(actual[i]) or not lower <= actual[i] <= upper):
                errors.append(f'independent CMG {name}/{i} numerical certificate bound failed')
    inputs = diagnostics['moment_inputs']
    weights = inputs['weights']
    residual = [number(result['columns'][expected['outputs']['residual']][i]) for i in expected['rows']]
    columns = list(inputs['regressors'])
    for effect in inputs['effects']:
        columns.extend([[float(value == level) for value in effect] for level in sorted(set(effect))])
    scale = max(1., max(abs(value) for value in residual))
    for column in columns:
        terms = [weight * value * error for weight, value, error in zip(weights, column, residual)]
        denominator = max(1., math.fsum(abs(weight * value) for weight, value in zip(weights, column)) * scale)
        moment = abs(math.fsum(terms)) / denominator
        if not math.isfinite(moment) or moment > diagnostics['tolerance']:
            errors.append('independent CMG original-design residual moment failed')
            break
    return errors
