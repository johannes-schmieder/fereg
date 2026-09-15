"""Independent development audit kernels; never a production estimation path.

The bounded admission datasets have positive outcomes, no weights/offsets and
one to three categorical intercept effects. Unsupported inputs fail closed.
These diagnostics do not adjudicate a comparator or activate automatic routing.
"""

import math
import argparse
import csv
import hashlib
import json
from pathlib import Path

import numpy as np
from scipy import sparse
from scipy.sparse.csgraph import connected_components
from scipy.sparse.linalg import lsmr

SOLVER_COLUMNS = ('requested_family selected_family selected_engine effective_family '
                  'effective_engine checkpoint_family row_engine ordinary_frames checkpoint_frames '
                  'retries fallback_count fallback_reason iterations setup_seconds solve_seconds '
                  'certification_seconds absinf_max relinf_max rel2_max normal_equation_max '
                  'projection_absinf_max backward_error_max').split()


def compare_optimum_intervals(candidate, reference_tokens, optimum_intervals, *, rtol, atol):
    """Check a proven optimum enclosure without turning it into admission.

    Every uncertain pairwise mismatch must have a separately proven reference
    error. Two estimates merely lying near the same enclosure is insufficient.
    """
    from decimal import Decimal
    from ppml_coefficient_accuracy import LOW, HIGH, point
    candidate = np.asarray(candidate, dtype=np.float64)
    rt, at = Decimal(rtol), Decimal(atol)
    if (candidate.ndim != 1 or not 1 <= candidate.size <= 2500
            or not np.isfinite(candidate).all()
            or len(reference_tokens) != candidate.size or len(optimum_intervals) != candidate.size
            or any(not v.is_finite() or v < 0 for v in (rt, at))):
        raise ValueError('invalid optimum interval comparison')
    def absolute_min(lo, hi):
        return Decimal(0) if lo <= 0 <= hi else min(lo.copy_abs(), hi.copy_abs())
    cells = []
    for j, (value, token, interval) in enumerate(zip(candidate, reference_tokens, optimum_intervals)):
        if len(interval) != 2:
            raise ValueError('invalid optimum interval width')
        lo, hi = map(Decimal, interval)
        ref = Decimal(token)
        if (not all(v.is_finite() for v in (lo, hi, ref)) or lo > hi
                or abs(ref.as_tuple().exponent) > 300):
            raise ValueError('invalid optimum or reference interval')
        c = point(value)[0]
        unit = Decimal((0, (1,), ref.as_tuple().exponent))
        rl, rh = LOW.subtract(ref, unit), HIGH.add(ref, unit)
        c_distance = max(HIGH.subtract(c, lo), HIGH.subtract(hi, c))
        c_tolerance = LOW.add(at, LOW.multiply(rt, max(c.copy_abs(), absolute_min(lo, hi))))
        r_distance = max(Decimal(0), LOW.subtract(lo, rh), LOW.subtract(rl, hi))
        r_tolerance = HIGH.add(at, HIGH.multiply(rt,
            max(lo.copy_abs(), hi.copy_abs(), rl.copy_abs(), rh.copy_abs())))
        pair_distance = max(HIGH.subtract(c, rl), HIGH.subtract(rh, c))
        pair_tolerance = LOW.add(at, LOW.multiply(rt, max(c.copy_abs(), absolute_min(rl, rh))))
        candidate_pass = c_distance <= c_tolerance
        reference_outside = r_distance > r_tolerance
        pair_pass = pair_distance <= pair_tolerance
        cells.append({'coordinate': j+1, 'candidate_inside_optimum_envelope': candidate_pass,
                      'reference_outside_optimum_envelope': reference_outside,
                      'pairwise_envelope_proven': pair_pass,
                      'candidate_distance_upper': str(c_distance),
                      'reference_distance_lower': str(r_distance),
                      'unresolved': not candidate_pass or not (pair_pass or reference_outside)})
    return {'campaign_admission': False,
            'requires_source_bound_optimum_certificate': True,
            'requires_all_other_scientific_gates': True,
            'candidate_inside_optimum_envelope': all(c['candidate_inside_optimum_envelope'] for c in cells),
            'reference_accuracy_exception_supported': (not any(c['unresolved'] for c in cells)
                and any(not c['pairwise_envelope_proven'] for c in cells)),
            'unresolved_coordinates': [c['coordinate'] for c in cells if c['unresolved']],
            'coordinates': cells}


def compare_reference_coefficients(beta, reference_tokens, radius_upper):
    """Compare reported decimal coefficients with a separately certified radius.

    One last printed decimal unit encloses reference serialization uncertainty.
    This calculation alone validates neither the proof nor the reference input.
    """
    from decimal import Decimal
    from ppml_coefficient_accuracy import LOW, HIGH, point
    beta = np.asarray(beta)
    radius = Decimal(radius_upper)
    if (beta.ndim != 1 or not 1 <= len(beta) <= 50 or len(reference_tokens) != len(beta)
            or not np.isfinite(beta).all() or not radius.is_finite()
            or not Decimal(0) <= radius <= Decimal('1e-10')):
        raise ValueError('invalid certified coefficient comparison')
    result = []
    for j, (candidate, token) in enumerate(zip(beta, reference_tokens)):
        reference = Decimal(token)
        if not reference.is_finite() or abs(reference.as_tuple().exponent) > 300:
            raise ValueError('invalid reference coefficient token')
        printed_unit = Decimal((0, (1,), reference.as_tuple().exponent))
        candidate = point(candidate)[0]
        if reference >= candidate:
            gap = LOW.subtract(reference, candidate)
        else:
            gap = LOW.subtract(candidate, reference)
        distance_lower = max(Decimal(0), LOW.subtract(LOW.subtract(gap, radius), printed_unit))
        scale = max(HIGH.add(reference.copy_abs(), printed_unit),
                    HIGH.add(candidate.copy_abs(), radius))
        tolerance = HIGH.add(Decimal('1e-10'), HIGH.multiply(Decimal('1e-7'), scale))
        result.append({'control': j+1, 'optimum_distance_lower': str(distance_lower),
                       'comparison_tolerance_upper': str(tolerance),
                       'outside_certified_optimum_envelope': distance_lower > tolerance})
    return result


def read_capture(prefix, *, controls):
    """Read complete scalar/matrix evidence; duplicate or unknown cells fail."""
    prefix = Path(prefix)
    scalar_names = {'N', 'll', 'deviance', 'rank', 'df_a', 'df_resid', 'converged',
                    'feppml_threads', 'feppml_solver_fallback_count', 'outer_tolerance'}
    shapes = {'b': (1, controls + 1), 'V': (controls + 1, controls + 1),
              'feppml_certificate': (16, 1), 'feppml_solver': (4, 22)}
    scalars, matrices, columns, cells = {}, {}, {}, set()
    for name, shape in shapes.items():
        matrices[name] = np.full(shape, np.nan)
    with Path(str(prefix) + '.tsv').open() as stream:
        for fields in csv.reader(stream, delimiter='\t'):
            if len(fields) == 3 and fields[0] == 'scalar':
                _, name, value = fields
                if name not in scalar_names or name in scalars:
                    raise ValueError('unknown or duplicate scalar')
                scalars[name] = float(value)
            elif len(fields) == 3 and fields[0] == 'columns':
                _, name, value = fields
                if name not in shapes or name in columns:
                    raise ValueError('unknown or duplicate stripe')
                columns[name] = value.split()
                if len(columns[name]) != shapes[name][1]:
                    raise ValueError('invalid stripe width')
            elif len(fields) == 5 and fields[0] == 'matrix':
                _, name, row, col, value = fields
                row, col = int(row), int(col)
                if (name not in shapes or not 1 <= row <= shapes[name][0]
                        or not 1 <= col <= shapes[name][1] or (name, row, col) in cells):
                    raise ValueError('unknown, duplicate or out-of-range matrix cell')
                cells.add((name, row, col))
                # Inactive engine diagnostics legitimately publish Stata missing.
                matrices[name][row-1, col-1] = np.nan if value.strip() == '.' else float(value)
            else:
                raise ValueError('invalid capture record')
    if (set(scalars) != scalar_names or set(columns) != set(shapes)
            or len(cells) != sum(r*c for r, c in shapes.values())
            or not all(math.isfinite(v) for v in scalars.values())):
        raise ValueError('incomplete or nonfinite capture')
    expected = [f'x{j+1}' for j in range(controls)] + ['o._cons']
    if columns['b'] != expected or columns['V'] != expected:
        raise ValueError('coefficient status/order mismatch')
    if columns['feppml_certificate'] != ['value'] or columns['feppml_solver'] != SOLVER_COLUMNS:
        raise ValueError('certificate or solver schema mismatch')
    for name in ('b', 'V', 'feppml_certificate'):
        if not np.isfinite(matrices[name]).all():
            raise ValueError('nonfinite scientific matrix')
    return scalars, matrices


def audit_stability(default_prefix, tighter_prefix, *, y, x, groups, threads, ring_multiplier=None):
    """Require independent audits of both states before comparing stability.

    Outer tolerances do not identify the actual checkpoint recurrence target.
    Profile rejection may refine that target; independently audit both states
    rather than inferring their accuracy from a solver schedule.
    """
    reports = [audit_capture(p, y=y, x=x, groups=groups, threads=threads,
                             ring_multiplier=ring_multiplier)
               for p in (default_prefix, tighter_prefix)]
    k = np.asarray(x).shape[1]
    left, lm = read_capture(default_prefix, controls=k)
    right, rm = read_capture(tighter_prefix, controls=k)
    if left['outer_tolerance'] != 1e-8 or right['outer_tolerance'] != 1e-10:
        raise ValueError('stability requires default and tighter outer tolerances')
    differences = {}
    for name, rtol, atol in (('b', 1e-7, 1e-10), ('V', 1e-6, 1e-10)):
        if not np.allclose(lm[name], rm[name], rtol=rtol, atol=atol):
            raise ValueError(f'{name} tighter-outer stability failed')
        differences[name] = float(np.max(np.abs(lm[name] - rm[name])))
    for name in ('ll', 'deviance'):
        if not math.isclose(left[name], right[name], rel_tol=1e-10, abs_tol=1e-8):
            raise ValueError('objective tighter-outer stability failed')
        differences[name] = abs(left[name] - right[name])
    means = [np.genfromtxt(str(p)+'.csv', delimiter=',', names=True)['audit_mu']
             for p in (default_prefix, tighter_prefix)]
    if not np.allclose(*means, rtol=1e-7, atol=1e-9):
        raise ValueError('prediction tighter-outer stability failed')
    differences['mu'] = float(np.max(np.abs(means[0] - means[1])))
    return {'outer_score_stability_pass': True,
            'checkpoint_recurrence_tolerances_observed': False,
            'differences': differences, 'independent_audits': reports}


def audit_capture(prefix, *, y, x, groups, threads, ring_multiplier=None):
    """Audit a capture against independently supplied original generated data.

    The runner must separately bind source/plugin, input hashes, command and
    terminal process evidence. This function never confers campaign admission.
    """
    y, x, groups = (np.asarray(a) for a in (y, x, groups))
    n, k = x.shape
    scalars, matrices = read_capture(prefix, controls=k)
    path = Path(str(prefix) + '.csv')
    data = np.genfromtxt(path, delimiter=',', names=True)
    names = ['audit_row', 'audit_sample', 'y'] + [f'x{j+1}' for j in range(k)]
    names += [f'g{j+1}' for j in range(groups.shape[1])] + ['audit_mu']
    if data.shape != (n,) or list(data.dtype.names) != names:
        raise ValueError('capture sample dimensions or fields differ')
    if (not np.array_equal(data['audit_row'], np.arange(1, n+1))
            or not np.all(data['audit_sample'] == 1) or not np.array_equal(data['y'], y)):
        raise ValueError('capture sample/outcome differs from input')
    for j in range(k):
        if not np.array_equal(data[f'x{j+1}'], x[:, j]):
            raise ValueError('capture controls differ from input')
    for j in range(groups.shape[1]):
        if not np.array_equal(data[f'g{j+1}'], groups[:, j]):
            raise ValueError('capture groups differ from input')
    dof = categorical_dof(groups)
    if (scalars['N'] != n or scalars['converged'] != 1 or scalars['rank'] != k+1
            or scalars['outer_tolerance'] not in (1e-8, 1e-10)
            or scalars['df_a'] != dof['df_a'] or scalars['feppml_threads'] != threads
            or scalars['df_resid'] != n-(k+1)-dof['df_a']
            or scalars['feppml_solver_fallback_count'] != 0
            or matrices['feppml_certificate'][-1, 0] != 1):
        raise ValueError('sample/rank/DoF/runtime certificate mismatch')
    solver = matrices['feppml_solver']
    family = 2 if groups.shape[1] == 1 else 3
    engine = 1 if family == 2 else (2 if k == 0 else 3)
    if (not np.all(solver[:, 0] == 1) or not np.all(solver[:, 1] == family)
            or not np.all(solver[:, 3] == family)
            or not np.all(solver[:, 2] == engine) or not np.all(solver[:, 4] == engine)
            or not np.all(solver[:, 5] == family)
            or not np.array_equal(solver[:, 6], np.arange(1, 5))
            or not np.isfinite(solver[:, :16]).all() or np.any(solver[:, :16] < 0)
            or np.any(solver[:, :13] != np.floor(solver[:, :13]))
            or not np.all(solver[:, 10:12] == 0)):
        raise ValueError('unexpected automatic route or fallback')
    beta = matrices['b'][0, :k]
    if (matrices['b'][0, k] != 0 or np.any(matrices['V'][k, :] != 0)
            or np.any(matrices['V'][:, k] != 0)):
        raise ValueError('omitted constant result mismatch')
    binding = coefficient_binding(x, beta, data['audit_mu'], groups, ring_multiplier=ring_multiplier)
    score = objective_score(y, x, data['audit_mu'], groups)
    if not binding['coefficient_binding_pass'] or not score['stationarity_pass']:
        raise ValueError('independent predictor/score audit failed: '+json.dumps(
            {'binding': binding, 'score': score}, sort_keys=True, allow_nan=False))
    covariance = control_covariance(y, x, data['audit_mu'], groups, ring_multiplier=ring_multiplier)
    for reported, replayed in ((scalars['ll'], score['log_likelihood']),
                               (scalars['deviance'], score['deviance'])):
        if not math.isclose(reported, replayed, rel_tol=1e-10, abs_tol=1e-8):
            raise ValueError('objective replay mismatch')
    oracle_v = np.asarray(covariance['control_covariance']).reshape(k, k)
    if not np.allclose(matrices['V'][:k, :k], oracle_v, rtol=1e-6, atol=1e-10):
        raise ValueError('independent control covariance mismatch')
    return {'independent_candidate_pass': True, 'binding': binding, 'score': score,
            'covariance': covariance, 'dof': dof,
            'capture_sha256': {suffix: hashlib.sha256(Path(str(prefix)+suffix).read_bytes()).hexdigest()
                               for suffix in ('.csv', '.tsv')}}


def _indicator_design(groups):
    n, effects = groups.shape
    codes = []
    offset = 0
    for effect in range(effects):
        levels, inverse = np.unique(groups[:, effect], return_inverse=True)
        codes.append(inverse + offset)
        offset += levels.size
    return sparse.coo_matrix(
        (np.ones(n * effects), (np.tile(np.arange(n), effects), np.concatenate(codes))),
        shape=(n, offset)).tocsr()


def categorical_dof(groups):
    """Replay the pairwise categorical DoF policy, not three-way dummy rank."""
    groups = np.asarray(groups)
    if (groups.ndim != 2 or groups.shape[0] < 2
            or not 1 <= groups.shape[1] <= 3 or not np.isfinite(groups).all()
            or np.any(groups != np.floor(groups))):
        raise ValueError("unsupported categorical DoF inputs")
    partitions = [np.unique(groups[:, j], return_inverse=True)
                  for j in range(groups.shape[1])]
    counts = [len(levels) for levels, _ in partitions]
    redundant = [1]  # The published structural constant owns one FE direction.
    for later in range(1, len(partitions)):
        maximum = 0
        for earlier in range(later):
            left = partitions[earlier][1]
            right = partitions[later][1] + counts[earlier]
            size = counts[earlier] + counts[later]
            graph = sparse.coo_matrix((np.ones(len(left)), (left, right)),
                                       shape=(size, size)).tocsr()
            components = connected_components(graph, directed=False, return_labels=False)
            maximum = max(maximum, int(components))
        redundant.append(maximum)
    return {"categories": counts, "redundant": redundant,
            "df_a": sum(counts) - sum(redundant),
            "pairwise_approximation": len(partitions) == 3}


def control_covariance(y, x, mu, groups, *, memory_bytes=512 * 1024**2,
                       max_iterations=100000, ring_multiplier=None):
    """Independent exact-weight FWL robust covariance for full-rank controls.

    This reprojects every control with SciPy, independently of fereg's saved
    bread/projections. It does not certify the reported normalization constant's
    covariance. Weak-graph projection accuracy is numeric evidence, not a
    conditioning-independent forward-error bound.
    """
    x, groups = np.asarray(x), np.asarray(groups)
    if x.ndim != 2 or groups.ndim != 2:
        raise ValueError("unsupported covariance dimensions")
    n, k = x.shape
    modeled_bytes = 8 * (6 * x.size + 96 * n * groups.shape[1] + 48 * n + 12 * k*k)
    if memory_bytes < modeled_bytes:
        raise MemoryError("independent covariance scratch exceeds audit budget")
    # A reconstructed covariance is not a stationarity certificate; the caller
    # must separately require objective_score's stationarity_pass.
    objective_score(y, x, mu, groups)
    y, x, mu, groups = (np.asarray(a, dtype=float) for a in (y, x, mu, groups))
    if not isinstance(max_iterations, int) or max_iterations < 1:
        raise ValueError("invalid projection iteration budget")
    design = _indicator_design(groups)
    sqrt_mu = np.sqrt(mu)
    weighted = design.multiply(sqrt_mu[:, None]).tocsr()
    norms = np.sqrt(np.asarray(design.T @ mu))
    within = np.empty_like(x)
    diagnostics = []
    if ring_multiplier is not None:
        if groups.shape[1] != 2 or not isinstance(ring_multiplier, int):
            raise ValueError('direct ring audit requires two effects and an integer permutation')
        from ppml_coefficient_accuracy import _ring_project
        if k:
            within, _, _ = _ring_project(groups, mu, x, ring_multiplier, memory_bytes)
    for column in range(k):
        rhs = sqrt_mu * x[:, column]
        if ring_multiplier is None:
            solve = lsmr(weighted, rhs, atol=1e-14, btol=1e-14,
                         conlim=1e14, maxiter=max_iterations)
            within[:, column] = x[:, column] - design @ solve[0]
        moments = np.asarray(design.T @ (mu * within[:, column]))
        maximum = float(np.max(np.abs(moments) / norms)
                        / max(1.0, float(np.linalg.norm(rhs))))
        converged = ring_multiplier is not None or int(solve[1]) in (0, 1, 2, 4, 5)
        diagnostics.append({"iterations": int(solve[2]) if ring_multiplier is None else 0,
                            "stop": int(solve[1]) if ring_multiplier is None else None,
                            "method": 'lsmr' if ring_multiplier is None else 'direct_ring',
                            "fresh_normal_max": maximum})
        if not converged or not math.isfinite(maximum) or maximum > 1e-12:
            raise ValueError("independent control projection did not certify")
    hessian = within.T @ (mu[:, None] * within)
    if k:
        original_squares = (x * x).T @ mu
        if not np.isfinite(original_squares).all() or np.any(original_squares <= 0):
            raise ValueError("independent original control scale unresolved")
        scales = np.sqrt(original_squares)
        eigenvalues = np.linalg.eigvalsh(hessian / scales[:, None] / scales[None, :])
        if (not np.isfinite(eigenvalues).all() or eigenvalues[0] <= 0
                or eigenvalues[0] <= max(1.0, eigenvalues[-1]) * 1e-12):
            raise ValueError("independent finite rank unresolved")
        bread = np.linalg.solve(hessian, np.eye(k))
        inverse_error = float(np.max(np.abs(hessian @ bread - np.eye(k))))
        if not math.isfinite(inverse_error) or inverse_error > 1e-10:
            raise ValueError("independent finite inverse did not certify")
        scores = within * (y - mu)[:, None]
        covariance = n / (n - 1) * bread @ (scores.T @ scores) @ bread
        if not np.isfinite(covariance).all():
            raise ValueError("nonfinite independent covariance")
    else:
        covariance = np.empty((0, 0))
        inverse_error = 0.0
    return {"control_covariance": covariance.tolist(), "finite_rank": k,
            "inverse_error": inverse_error, "projections": diagnostics,
            "modeled_scratch_bytes": modeled_bytes}


def objective_score(y, x, mu, groups):
    """Replay positive, unweighted public-state objectives and every FE score."""
    y, x, mu, groups = (np.asarray(a, dtype=float) for a in (y, x, mu, groups))
    if (y.ndim != 1 or mu.shape != y.shape or x.ndim != 2
            or groups.ndim != 2 or x.shape[0] != y.size
            or groups.shape[0] != y.size or y.size < 2
            or not 1 <= groups.shape[1] <= 3 or x.shape[1] > 50
            or not all(np.isfinite(a).all() for a in (y, x, mu, groups))
            or np.any(y <= 0) or np.any(mu <= 0)
            or np.any(groups != np.floor(groups))):
        raise ValueError("unsupported objective/score audit inputs")
    residual = y - mu
    scale = max(float(np.linalg.norm(residual / np.sqrt(mu))),
                float(np.linalg.norm(y / np.sqrt(mu))))
    finite = np.column_stack((x, np.ones(y.size)))
    moments = finite.T @ residual
    squares = (finite * finite).T @ mu
    if (not np.isfinite(moments).all() or not np.isfinite(squares).all()
            or not math.isfinite(scale) or scale <= 0
            or np.any((squares == 0) & np.any(finite != 0, axis=0))):
        raise ValueError("nonfinite finite-score intermediates")
    # A zero design column contributes an exactly zero score, not 0/0.
    denominator = np.sqrt(squares) * scale
    if not np.isfinite(denominator).all():
        raise ValueError("nonfinite finite-score denominator")
    relative = np.divide(np.abs(moments), denominator,
                         out=np.zeros_like(moments), where=squares > 0)
    fe_maxima = []
    for effect in range(groups.shape[1]):
        _, codes = np.unique(groups[:, effect], return_inverse=True)
        fe_moments = np.bincount(codes, weights=residual)
        fe_squares = np.bincount(codes, weights=mu)
        if (not np.isfinite(fe_moments).all() or not np.isfinite(fe_squares).all()
                or np.any(fe_squares <= 0)):
            raise ValueError("nonfinite FE-score intermediates")
        denominator = np.sqrt(fe_squares) * scale
        if not np.isfinite(denominator).all():
            raise ValueError("nonfinite FE-score denominator")
        fe_maxima.append(float(np.max(np.abs(fe_moments) / denominator)))
    # log1p avoids cancellation near y == mu; the ordinary log ratio avoids
    # rounding (mu-y)/y to -1 for a very small but positive predicted mean.
    delta = (mu - y) / y
    log_ratio = np.log(mu) - np.log(y)
    near = np.abs(delta) < 0.5
    log_ratio[near] = np.log1p(delta[near])
    deviance = 2 * math.fsum((mu - y - y * log_ratio).tolist())
    ll = math.fsum(float(yi * math.log(mi) - mi - math.lgamma(yi + 1))
                   for yi, mi in zip(y, mu))
    finite_max = float(np.max(relative))
    values = [ll, deviance, scale, finite_max, *fe_maxima]
    if not all(math.isfinite(value) for value in values) or deviance < 0:
        raise ValueError("nonfinite or invalid objective/score replay")
    return {"log_likelihood": ll, "deviance": deviance,
            "score_scale": scale, "finite_score_max": finite_max,
            "fe_score_maxima": fe_maxima,
            "stationarity_pass": max(finite_max, *fe_maxima) <= 1e-8}


def coefficient_binding(x, beta, mu, groups, *, memory_bytes=512 * 1024**2,
                        max_iterations=100000, ring_multiplier=None):
    """Project log(mu)-X beta onto FE indicators, holding published beta fixed.

    Two categorical effects first use an independent grounded forest witness.
    Other cases keep their redundant-coordinate projection. Every acceptance
    residual is freshly reconstructed, not taken from recurrence diagnostics.
    """
    x = np.asarray(x)
    beta = np.asarray(beta)
    mu = np.asarray(mu)
    groups = np.asarray(groups)
    if (x.ndim != 2 or beta.shape != (x.shape[1],) or mu.ndim != 1
            or groups.ndim != 2 or x.shape[0] != mu.size
            or groups.shape[0] != mu.size or mu.size < 2
            or not 1 <= groups.shape[1] <= 3 or x.shape[1] > 50):
        raise ValueError("unsupported coefficient-binding dimensions")
    if (not all(np.isfinite(a).all() for a in (x, beta, mu, groups))
            or np.any(mu <= 0) or np.any(groups != np.floor(groups))):
        raise ValueError("invalid coefficient-binding values")
    n, effects = groups.shape
    # Conservative scratch envelope includes conversion, sorting, COO/CSR
    # construction and recurrence vectors, with the worst-case n levels/FE.
    modeled_bytes = 8 * (4 * x.size + 64 * n * effects + 32 * n)
    if memory_bytes < modeled_bytes:
        raise MemoryError("independent projection scratch exceeds audit budget")
    if not isinstance(max_iterations, int) or max_iterations < 1:
        raise ValueError("invalid projection iteration budget")
    target = np.log(mu.astype(float)) - np.asarray(x, dtype=float) @ beta
    if groups.shape[1] == 2 and ring_multiplier is None:
        from two_fe_span_audit import audit_two_fe_span
        result = audit_two_fe_span(target, groups, memory_bytes=memory_bytes,
                                  max_iterations=max_iterations)
        result['modeled_scratch_bytes'] = modeled_bytes
        return result
    design = _indicator_design(groups)
    if ring_multiplier is None:
        solution = lsmr(design, target, atol=1e-13, btol=1e-13,
                        conlim=1e14, maxiter=max_iterations)
        residual = target - design @ solution[0]
    else:
        if groups.shape[1] != 2 or not isinstance(ring_multiplier, int):
            raise ValueError('direct ring audit requires two effects and an integer permutation')
        from ppml_coefficient_accuracy import _ring_project
        residual, _, _ = _ring_project(groups, np.ones(n), target[:, None], ring_multiplier, memory_bytes)
        residual = residual[:, 0]
    scale = max(1.0, float(np.linalg.norm(target)))
    relative_residual = float(np.linalg.norm(residual) / scale)
    moments = np.asarray(design.T @ residual)
    counts = np.asarray(design.sum(axis=0)).ravel()
    normal_max = float(np.max(np.abs(moments) / np.sqrt(counts)) / scale)
    recurrence_converged = int(solution[1]) in (0, 1, 2, 4, 5) if ring_multiplier is None else None
    return {
        "method": 'lsmr' if ring_multiplier is None else 'direct_ring',
        "recurrence_stop": int(solution[1]) if ring_multiplier is None else None,
        "iterations": int(solution[2]) if ring_multiplier is None else 0,
        "recurrence_converged": recurrence_converged,
        "fresh_residual_relative": relative_residual,
        "fresh_normal_max": normal_max,
        "modeled_scratch_bytes": modeled_bytes,
        "coefficient_binding_pass": bool((recurrence_converged or ring_multiplier is not None)
                                         and relative_residual <= 1e-10
                                         and normal_max <= 1e-12),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input-csv', type=Path, required=True)
    parser.add_argument('--capture-prefix', type=Path, required=True)
    parser.add_argument('--tighter-prefix', type=Path)
    parser.add_argument('--ring-multiplier', type=int)
    parser.add_argument('--controls', type=int, choices=range(51), required=True)
    parser.add_argument('--effects', type=int, choices=(1, 2, 3), required=True)
    parser.add_argument('--threads', type=int, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.threads < 1 or args.output.exists():
        parser.error('positive threads and a new output path are required')
    data = np.genfromtxt(args.input_csv, delimiter=',', names=True)
    x = np.column_stack([data[f'x{j+1}'] for j in range(args.controls)]) if args.controls else np.empty((len(data), 0))
    groups = np.column_stack([data[f'g{j+1}'] for j in range(args.effects)])
    if args.tighter_prefix:
        result = audit_stability(args.capture_prefix, args.tighter_prefix,
                                 y=data['y'], x=x, groups=groups, threads=args.threads,
                                 ring_multiplier=args.ring_multiplier)
    else:
        result = audit_capture(args.capture_prefix, y=data['y'], x=x, groups=groups,
                               threads=args.threads, ring_multiplier=args.ring_multiplier)
    result.update(schema_version=1, qualification_eligible=False,
                  campaign_admission=False,
                  input_csv_sha256=hashlib.sha256(args.input_csv.read_bytes()).hexdigest(),
                  audit_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    with args.output.open('x') as stream:
        json.dump(result, stream, indent=2, allow_nan=False)
        stream.write('\n')
    print('INDEPENDENT_AUTO_AUDIT_PASS')


if __name__ == '__main__':
    main()
