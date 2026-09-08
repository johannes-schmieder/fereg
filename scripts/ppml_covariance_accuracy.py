"""Independent fixed-state covariance error propagation, not admission policy.

Projection/Gram error bounds must be independently established for the same
state. This module does not infer their validity from small numerical residuals.
"""

from decimal import Decimal
import hashlib
from pathlib import Path
import time

import numpy as np

from ppml_coefficient_accuracy import LOW, HIGH, ZERO, point, add, sub, mul, tree, energy_bound, _ring_project


def _absolute(interval):
    return max(interval[0].copy_abs(), interval[1].copy_abs())


def _sqrt_upper(value):
    return HIGH.next_plus(HIGH.sqrt(value))


def _frobenius_upper(matrix):
    total = Decimal(0)
    for row in matrix:
        for value in row:
            absolute = _absolute(value)
            total = HIGH.add(total, HIGH.multiply(absolute, absolute))
    return _sqrt_upper(total)


def _operator_upper(matrix):
    def maximum_sum(rows):
        maximum = Decimal(0)
        for row in rows:
            total = Decimal(0)
            for value in row:
                total = HIGH.add(total, _absolute(value))
            maximum = max(maximum, total)
        return maximum
    return min(_frobenius_upper(matrix), _sqrt_upper(HIGH.multiply(
        maximum_sum(matrix), maximum_sum(zip(*matrix)))))


def _product(left, right):
    result = []
    for row in left:
        output = []
        for column in zip(*right):
            value = ZERO
            for a, b in zip(row, column):
                value = add(value, mul(a, b))
            output.append(value)
        result.append(output)
    return result


def _gram_enclosure(values, weights):
    n, k = values.shape
    if (n > 2**20 or not np.isfinite(values).all() or not np.isfinite(weights).all()
            or np.any(weights < 2.0**-100) or np.any(weights > 2.0**100)
            or np.any((values != 0) & (np.abs(values) < 2.0**-100))
            or np.any(np.abs(values) > 2.0**100)):
        raise ValueError('FP64 covariance Gram arithmetic envelope exceeded')
    gram = values.T @ (weights[:, None]*values)
    absolute = np.abs(values).T @ (weights[:, None]*np.abs(values))
    if not np.isfinite(gram).all() or not np.isfinite(absolute).all():
        raise ValueError('nonfinite covariance Gram')
    unit = HIGH.divide(Decimal(1), Decimal(2**53))
    numerator = HIGH.multiply(Decimal(3*n), unit)
    gamma = HIGH.divide(numerator, LOW.subtract(Decimal(1), numerator))
    amplifier = HIGH.divide(Decimal(1), LOW.subtract(Decimal(1), gamma))
    # Copy one triangle exactly; do not average and introduce another rounding.
    symmetric = np.tril(gram) + np.tril(gram, -1).T
    error = HIGH.multiply(Decimal(k), HIGH.multiply(gamma,
        HIGH.multiply(amplifier, point(float(absolute.max()))[0])))
    trace = Decimal(0)
    for value in np.diag(absolute):
        trace = HIGH.add(trace, HIGH.multiply(amplifier, point(value)[0]))
    return symmetric, error, trace


def fixed_mean_covariance_accuracy(y, x, mu, groups, *, firm_multiplier,
                                   memory_bytes=1024**3, optimum_log_error_upper=None):
    """Certify fixed-mean V, optionally propagating an independently proven log bound.

    Supplying a log bound is not proof of that bound. Admission must bind it to
    a full-model optimum certificate on these exact input arrays and means.
    """
    start = time.monotonic()
    y, x, mu, groups = [np.asarray(a, dtype=np.float64) for a in (y, x, mu, groups)]
    if (x.ndim != 2 or not 1 <= x.shape[1] <= 50 or not 8 <= x.shape[0] <= 75000
            or y.shape != (x.shape[0],) or mu.shape != y.shape or groups.shape != (len(y), 2)
            or not all(np.isfinite(a).all() for a in (y, x, mu, groups))
            or np.any(y <= 0) or np.any(mu <= 0) or np.any(groups != np.floor(groups))
            or not isinstance(firm_multiplier, int)):
        raise ValueError('unsupported fixed-mean covariance surface')
    n, k = x.shape
    vertices = sum(len(np.unique(groups[:, j])) for j in range(2))
    modeled = 8*(220*n + 12*n*k + 12*vertices*(k+3) + 1000*k*k)
    if modeled > memory_bytes:
        raise MemoryError('fixed-mean covariance workspace exceeds budget')
    topology = tree(groups)
    r, ax, fx = _ring_project(groups, mu, x, firm_multiplier, memory_bytes)
    scores = r*(y-mu)[:, None]
    weights = [point(v) for v in mu]
    residual = [sub(point(yi), wi) for yi, wi in zip(y, weights)]
    c = Decimal(0)
    for value, weight in zip(residual, weights):
        radius = _absolute(value)
        c = max(c, HIGH.divide(HIGH.multiply(radius, radius), weight[0]))
    eps2 = Decimal(0)
    rounding_h2 = Decimal(0)
    rounding_m2 = Decimal(0)
    energies = []
    for j in range(k):
        loads = []
        for i in range(n):
            exact_r = sub(sub(point(x[i,j]), point(ax[i,j])), point(fx[i,j]))
            loads.append(mul(weights[i], exact_r))
            rh = _absolute(sub(exact_r, point(r[i,j])))
            rm = _absolute(sub(mul(exact_r, residual[i]), point(scores[i,j])))
            rounding_h2 = HIGH.add(rounding_h2, HIGH.multiply(weights[i][1], HIGH.multiply(rh, rh)))
            rounding_m2 = HIGH.add(rounding_m2, HIGH.multiply(rm, rm))
        energy = energy_bound(topology, loads, weights)
        eps2 = HIGH.add(eps2, energy)
        energies.append(str(energy))
    h, eh, trace_h = _gram_enclosure(r, mu)
    m, em, trace_m = _gram_enclosure(scores, np.ones(n))
    def gram_rounding(trace, squared):
        return HIGH.add(squared, HIGH.multiply(Decimal(2),
            HIGH.multiply(_sqrt_upper(trace), _sqrt_upper(squared))))
    eh = HIGH.add(HIGH.add(eh, gram_rounding(trace_h, rounding_h2)), eps2)
    score_norm = HIGH.add(_sqrt_upper(trace_m), _sqrt_upper(rounding_m2))
    score_projection2 = HIGH.multiply(c, eps2)
    projection_m = HIGH.add(score_projection2, HIGH.multiply(Decimal(2),
        HIGH.multiply(score_norm, _sqrt_upper(score_projection2))))
    em = HIGH.add(HIGH.add(em, gram_rounding(trace_m, rounding_m2)), projection_m)
    fixed_eh, fixed_em = eh, em
    if optimum_log_error_upper is not None:
        delta = Decimal(optimum_log_error_upper)
        if not delta.is_finite() or not Decimal(0) <= delta <= Decimal('0.1'):
            raise ValueError('unsupported optimum log-mean enclosure')
        t = HIGH.subtract(HIGH.next_plus(HIGH.exp(delta)), Decimal(1))
        a = LOW.next_minus(LOW.exp(delta.copy_negate()))
        kappa = HIGH.divide(t, a)
        hn = HIGH.add(_operator_upper([[point(v) for v in row] for row in h]), eh)
        mn = HIGH.add(_operator_upper([[point(v) for v in row] for row in m]), em)
        score_drift = HIGH.multiply(_sqrt_upper(hn), HIGH.add(
            HIGH.multiply(kappa, _sqrt_upper(c)),
            HIGH.multiply(t, HIGH.multiply(HIGH.add(Decimal(1), kappa),
                _sqrt_upper(max(w[1] for w in weights))))))
        # Profiled Hessian comparison follows weighted minimization, while
        # robust meat also depends on FE-residual and outcome-residual drift.
        eh = HIGH.add(eh, HIGH.multiply(t, hn))
        em = HIGH.add(em, HIGH.add(HIGH.multiply(score_drift, score_drift),
            HIGH.multiply(Decimal(2), HIGH.multiply(_sqrt_upper(mn), score_drift))))
    result = covariance_perturbation(h, m, np.linalg.inv(h), hessian_error_upper=eh,
                                    meat_error_upper=em, observations=n)
    result.update(fixed_binary_mean_covariance_certified=optimum_log_error_upper is None,
                  optimum_covariance_conditional_on_log_bound=optimum_log_error_upper is not None,
                  optimum_log_error_upper=str(optimum_log_error_upper) if optimum_log_error_upper is not None else None,
                  fixed_mean_hessian_error_upper=str(fixed_eh), fixed_mean_meat_error_upper=str(fixed_em),
                  hessian_error_upper=str(eh), meat_error_upper=str(em),
                  projection_energy_upper=energies, modeled_workspace_bytes=modeled,
                  elapsed_seconds=time.monotonic()-start,
                  arithmetic='IEEE FP64 guarded Gram; 60-digit outward Decimal',
                  proof_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  shared_interval_source_sha256=hashlib.sha256(Path(__file__).with_name('ppml_coefficient_accuracy.py').read_bytes()).hexdigest())
    result['arrays_sha256'] = {}
    for name, value in (('y',y), ('x',x), ('mu',mu), ('groups',groups), ('worker_potentials',ax), ('firm_potentials',fx)):
        a = np.ascontiguousarray(value)
        result['arrays_sha256'][name] = {'shape': list(a.shape), 'dtype': a.dtype.str,
                                        'sha256': hashlib.sha256(a.tobytes()).hexdigest()}
    return result


def covariance_perturbation(hessian, meat, inverse, *, hessian_error_upper,
                            meat_error_upper, observations):
    """Enclose q H^-1 M H^-1 from certified spectral H/M error bounds.

    The supplied inverse is an arbitrary approximation, not trusted evidence.
    Every small-matrix product and its inverse residual is outward enclosed.
    Full-model stationarity and accuracy of the chosen state are separate facts.
    """
    matrices = [np.asarray(a, dtype=np.float64) for a in (hessian, meat, inverse)]
    h, m, b = matrices
    if (h.ndim != 2 or h.shape[0] != h.shape[1] or not 1 <= h.shape[0] <= 50
            or any(a.shape != h.shape or not np.isfinite(a).all() for a in matrices)
            or not np.array_equal(h, h.T) or not np.array_equal(m, m.T)
            or not isinstance(observations, int) or observations < 2):
        raise ValueError('unsupported covariance perturbation inputs')
    eh, em = Decimal(hessian_error_upper), Decimal(meat_error_upper)
    if any(not e.is_finite() or e < 0 for e in (eh, em)):
        raise ValueError('invalid externally certified error bounds')
    hi, mi, bi = [[[point(value) for value in row] for row in a] for a in matrices]
    k = len(hi)
    curvature = None
    for j, row in enumerate(hi):
        off = Decimal(0)
        for i, value in enumerate(row):
            if i != j:
                off = HIGH.add(off, _absolute(value))
        lower = LOW.subtract(LOW.subtract(row[j][0], off), eh)
        curvature = lower if curvature is None else min(curvature, lower)
    if curvature <= 0:
        raise ValueError('true Hessian curvature not certified')
    product = _product(hi, bi)
    residual = [[sub(point(int(i == j)), product[i][j]) for j in range(k)] for i in range(k)]
    rho = _operator_upper(residual)
    bn = _operator_upper(bi)
    mn = HIGH.add(_operator_upper(mi), em)
    # H(B0-H^-1)=(H-H0)B0+(H0 B0-I), with ||H^-1|| <=1/curvature.
    delta = HIGH.divide(HIGH.add(rho, HIGH.multiply(eh, bn)), curvature)
    inverse_term = HIGH.multiply(HIGH.multiply(delta,
        HIGH.add(HIGH.multiply(Decimal(2), bn), delta)), mn)
    meat_term = HIGH.multiply(HIGH.multiply(bn, bn), em)
    correction = (LOW.divide(Decimal(observations), Decimal(observations-1)),
                  HIGH.divide(Decimal(observations), Decimal(observations-1)))
    error = HIGH.multiply(correction[1], HIGH.add(inverse_term, meat_term))
    center = _product(_product(bi, mi), list(map(list, zip(*bi))))
    intervals = []
    for row in center:
        output = []
        for value in row:
            value = mul(correction, value)
            output.append([str(LOW.subtract(value[0], error)),
                           str(HIGH.add(value[1], error))])
        intervals.append(output)
    return {'campaign_admission': False, 'state_accuracy_certified': False,
            'requires_independent_input_error_certificates': True,
            'hessian_curvature_lower': str(curvature),
            'inverse_residual_frobenius_upper': str(_frobenius_upper(residual)),
            'inverse_residual_operator_upper': str(rho),
            'inverse_error_spectral_upper': str(delta),
            'covariance_error_spectral_upper': str(error),
            'covariance_intervals': intervals}
