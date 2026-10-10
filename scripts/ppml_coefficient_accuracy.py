"""Independent bounded two-FE coefficient-accuracy certificate.

The direct ring solver supplies approximations only. Outward-rounded Decimal
residuals and tree flows certify them against an exactly feasible Poisson state.
The Gram enclosure assumes standard IEEE FP64 multiplication/addition/FMA;
explicit range guards exclude overflow and underflow. This is development
evidence, never a production estimator or automatic campaign admission.
"""

import hashlib
import platform
import time
from collections import deque
from decimal import Decimal, Context, ROUND_FLOOR, ROUND_CEILING
from pathlib import Path

import numpy as np
import scipy
from scipy import sparse
from scipy.sparse.csgraph import connected_components
from scipy.linalg import cholesky_banded, cho_solve_banded, cho_factor, cho_solve

LOW = Context(prec=60, rounding=ROUND_FLOOR)
HIGH = Context(prec=60, rounding=ROUND_CEILING)
ZERO = (Decimal(0), Decimal(0))


def point(value):
    exact = Decimal.from_float(float(value))
    if not exact.is_finite():
        raise ValueError('nonfinite interval input')
    return exact, exact


def add(a, b):
    return LOW.add(a[0], b[0]), HIGH.add(a[1], b[1])


def sub(a, b):
    return LOW.subtract(a[0], b[1]), HIGH.subtract(a[1], b[0])


def mul(a, b):
    return (min(LOW.multiply(x, y) for x in a for y in b),
            max(HIGH.multiply(x, y) for x in a for y in b))


def exp(a):
    return LOW.next_minus(LOW.exp(a[0])), HIGH.next_plus(HIGH.exp(a[1]))


def tree(groups):
    return forest(groups, require_connected=True)


def forest(groups, *, require_connected=False):
    _, worker = np.unique(groups[:, 0], return_inverse=True)
    _, firm = np.unique(groups[:, 1], return_inverse=True)
    nw = int(worker.max())+1
    firm = firm+nw
    vertices = int(firm.max())+1
    adjacency = [[] for _ in range(vertices)]
    for i, (u, v) in enumerate(zip(worker, firm)):
        adjacency[u].append((int(v), i)); adjacency[v].append((int(u), i))
    parent = [-1]*vertices; edge = [-1]*vertices; order = []; roots = 0
    for root in range(vertices):
        if parent[root] != -1:
            continue
        roots += 1; parent[root] = root; order.append(root); queue = deque([root])
        while queue:
            u = queue.popleft()
            for v, i in adjacency[u]:
                if parent[v] == -1:
                    parent[v] = u; edge[v] = i; order.append(v); queue.append(v)
    if require_connected and roots != 1:
        raise ValueError('tree certificate requires connected graph')
    return worker, firm, parent, edge, order


def energy_bound(topology, edge_loads, weights):
    worker, firm, parent, edge, order = topology
    if len(edge_loads) != len(worker) or len(weights) != len(worker):
        raise ValueError('edge certificate length mismatch')
    loads = [ZERO for _ in parent]
    for u, v, value in zip(worker, firm, edge_loads):
        loads[u] = add(loads[u], value)
        loads[v] = sub(loads[v], value)
    energy = Decimal(0)
    for vertex in reversed(order):
        if parent[vertex] == vertex:
            continue
        value = loads[vertex]
        absolute = max(value[0].copy_abs(), value[1].copy_abs())
        lower_weight = weights[edge[vertex]][0]
        if lower_weight <= 0:
            raise ValueError('nonpositive tree weight enclosure')
        energy = HIGH.add(energy, HIGH.divide(HIGH.multiply(absolute, absolute), lower_weight))
        loads[parent[vertex]] = add(loads[parent[vertex]], value)
    for vertex in order:
        if parent[vertex] == vertex and not loads[vertex][0] <= 0 <= loads[vertex][1]:
            raise ValueError('component edge divergence does not balance')
    return energy


def _ring_project(groups, weights, values, firm_multiplier, memory_bytes):
    n, k = values.shape
    _, worker = np.unique(groups[:, 0], return_inverse=True)
    _, firm = np.unique(groups[:, 1], return_inverse=True)
    nw, nf = worker.max()+1, firm.max()+1
    if nf < 8 or 8*(20*n + 12*(nw+nf)*(k+3)) > memory_bytes:
        raise ValueError('unsupported size or memory envelope')
    firm = firm * firm_multiplier % nf
    if len(np.unique(firm)) != nf:
        raise ValueError('firm relabeling not bijective')
    mass = np.bincount(worker, weights=weights)
    cross = sparse.coo_matrix((weights, (worker, firm)), shape=(nw, nf)).tocsr()
    rows, columns, entries = [], [], []
    for w in range(nw):
        start, end = cross.indptr[w:w+2]
        vertices, edge_weights = cross.indices[start:end], cross.data[start:end]
        if len(vertices) > 4:
            raise ValueError('worker clique exceeds bounded envelope')
        for i in range(len(vertices)):
            for j in range(i):
                u, v = int(vertices[i]), int(vertices[j])
                if min(abs(u-v), nf-abs(u-v)) > 3:
                    raise ValueError('not a width-three cyclic graph')
                conductance = edge_weights[i]*edge_weights[j]/mass[w]
                rows.extend((u, v, u, v)); columns.extend((u, v, v, u))
                entries.extend((conductance, conductance, -conductance, -conductance))
    schur = sparse.coo_matrix((entries, (rows, columns)), shape=(nf, nf)).tocsr()
    # Ground firm 0. Keep firms1,2 as a border so the interior has width3.
    interior = schur[3:, 3:].tocsc()
    bands = np.zeros((4, nf-3))
    for diagonal in range(4):
        bands[diagonal, :nf-3-diagonal] = interior.diagonal(-diagonal)
    factor = cholesky_banded(bands, lower=True)
    coupling = schur[3:, 1:3].toarray()
    inverse_coupling = cho_solve_banded((factor, True), coupling)
    border = schur[1:3, 1:3].toarray() - coupling.T @ inverse_coupling
    border_factor = cho_factor(border, lower=True)
    worker_rhs = np.column_stack([np.bincount(worker, weights=weights*values[:, j], minlength=nw) for j in range(k)])
    firm_rhs = np.column_stack([np.bincount(firm, weights=weights*values[:, j], minlength=nf) for j in range(k)])
    rhs = firm_rhs - cross.T @ (worker_rhs/mass[:, None])
    base = cho_solve_banded((factor, True), rhs[3:])
    potentials = np.zeros((nf, k))
    potentials[1:3] = cho_solve(border_factor, rhs[1:3]-coupling.T @ base)
    potentials[3:] = base-inverse_coupling @ potentials[1:3]
    worker_potentials = (worker_rhs-cross @ potentials)/mass[:, None]
    residual = values-worker_potentials[worker]-potentials[firm]
    return residual, worker_potentials[worker], potentials[firm]


def coefficient_accuracy(y, x, beta, public_mu, groups, *, firm_multiplier,
                         memory_bytes=1024**3):
    return _coefficient_accuracy(y, x, beta, public_mu, groups,
        firm_multiplier=firm_multiplier, memory_bytes=memory_bytes,
        project=_ring_project, topology_builder=tree)


def two_fe_coefficient_accuracy(y, x, beta, public_mu, groups, *, memory_bytes=1024**3):
    """Bound arbitrary bounded two-FE graphs, including disconnected components.

    LAPACK supplies approximate potentials only; the same outward residual,
    forest-flow, curvature and Newton-ball calculations certify their accuracy.
    This is an independent diagnostic, never a comparator exception policy.
    """
    return _coefficient_accuracy(y, x, beta, public_mu, groups,
        firm_multiplier=1, memory_bytes=memory_bytes,
        project=_dense_reduced_project, topology_builder=forest)


def two_fe_nonnegative_sample_accuracy(y, x, beta, public_mu, groups, *, memory_bytes=1024**3):
    """Certify a supplied retained sample, including zero outcomes.

    The exponential Hessian and Newton-ball bound do not require y > 0. Sample
    selection must be established separately; this entry never drops rows or
    grants a comparator exception. The positive-outcome entries stay closed.
    """
    report = _coefficient_accuracy(y, x, beta, public_mu, groups,
        firm_multiplier=1, memory_bytes=memory_bytes,
        project=_dense_reduced_project, topology_builder=forest, allow_zeros=True)
    report.update(outcome_domain='nonnegative supplied sample',
                  zero_outcomes=int(np.count_nonzero(np.asarray(y) == 0)),
                  sample_selection_certified=False, finite_optimum_certified=True)
    return report


def _dense_reduced_project(groups, weights, values, firm_multiplier, memory_bytes):
    n, k = values.shape
    _, worker = np.unique(groups[:, 0], return_inverse=True)
    _, firm = np.unique(groups[:, 1], return_inverse=True)
    nw, nf = int(worker.max())+1, int(firm.max())+1
    if firm_multiplier != 1 or nf > 1200:
        raise ValueError('unsupported general two-FE projection size or relabeling')
    if 8*(20*n + 12*(nw+nf)*(k+3) + 5*nf*nf) > memory_bytes:
        raise MemoryError('general two-FE projection exceeds workspace envelope')
    mass = np.bincount(worker, weights=weights)
    cross = sparse.coo_matrix((weights, (worker, firm)), shape=(nw, nf)).tocsr()
    schur = np.diag(np.bincount(firm, weights=weights))
    schur -= (cross.T @ cross.multiply((1/mass)[:,None])).toarray()
    # Components come from incidence, not a threshold on weighted arithmetic.
    pattern = cross.copy(); pattern.data.fill(1.)
    count, labels = connected_components(pattern.T @ pattern)
    keep = np.ones(nf, dtype=bool)
    for component in range(count):
        keep[np.flatnonzero(labels == component)[0]] = False
    worker_rhs = np.column_stack([np.bincount(worker, weights=weights*values[:,j], minlength=nw) for j in range(k)])
    firm_rhs = np.column_stack([np.bincount(firm, weights=weights*values[:,j], minlength=nf) for j in range(k)])
    rhs = firm_rhs - cross.T @ (worker_rhs/mass[:,None])
    potentials = np.zeros((nf,k))
    if keep.any():
        factor = cho_factor(schur[np.ix_(keep,keep)], lower=True)
        potentials[keep] = cho_solve(factor, rhs[keep])
    worker_potentials = (worker_rhs - cross @ potentials)/mass[:,None]
    residual = values-worker_potentials[worker]-potentials[firm]
    return residual, worker_potentials[worker], potentials[firm]


def _coefficient_accuracy(y, x, beta, public_mu, groups, *, firm_multiplier,
                          memory_bytes, project, topology_builder, allow_zeros=False):
    y, x, beta, public_mu, g = (np.asarray(a, dtype=np.float64)
                                for a in (y, x, beta, public_mu, groups))
    if (x.ndim != 2 or not 1 <= x.shape[1] <= 50 or x.shape[0] > 75000
            or y.shape != (x.shape[0],) or public_mu.shape != y.shape
            or beta.shape != (x.shape[1],) or g.shape != (len(y), 2)
            or not all(np.isfinite(a).all() for a in (x,y,beta,public_mu,g))
            or np.any(y < 0) or (not allow_zeros and np.any(y == 0))
            or not np.any(y > 0) or np.any(public_mu <= 0)
            or np.any(g != np.floor(g)) or len(y) < 8):
        raise ValueError('unsupported coefficient-accuracy surface')
    if not isinstance(firm_multiplier, int):
        raise ValueError('firm multiplier must be an integer')
    # Covers interval loads/means, float designs/potentials and bounded cliques.
    n, k = x.shape
    vertices = sum(len(np.unique(g[:,j])) for j in range(2))
    modeled_bytes = 8*(200*n + 10*n*k + 12*vertices*(k+3))
    if project is _dense_reduced_project:
        modeled_bytes += 8*5*len(np.unique(g[:,1]))**2
    if modeled_bytes > memory_bytes:
        raise MemoryError('coefficient-accuracy workspace exceeds budget')
    z = np.log(public_mu)-x@beta
    _, worker_alpha, firm_alpha = project(g, np.ones(len(y)), z[:,None], firm_multiplier,
                                              memory_bytes)
    start = time.monotonic()
    beta_intervals = [point(b) for b in beta]
    weights, residual = [], []
    public_feasible_log_error = Decimal(0)
    for i, row in enumerate(x):
        eta = add(point(worker_alpha[i,0]), point(firm_alpha[i,0]))
        for j, value in enumerate(row):
            eta = add(eta, mul(point(value), beta_intervals[j]))
        mean = exp(eta)
        weights.append(mean); residual.append(sub(point(y[i]), mean))
        public = point(public_mu[i])[0]
        public_log = (LOW.next_minus(LOW.ln(public)), HIGH.next_plus(HIGH.ln(public)))
        log_error = sub(public_log, eta)
        public_feasible_log_error = max(public_feasible_log_error,
            log_error[0].copy_abs(), log_error[1].copy_abs())
    topology = topology_builder(g)
    fe_energy = energy_bound(topology, residual, weights)
    midpoint_mu = np.array([float((a+b)/2) for a,b in weights])
    # These are arbitrary floating approximations; later residual intervals certify
    # their projections against the true feasible state's enclosing weights.
    r, ax, fx = project(g, midpoint_mu, x, firm_multiplier, memory_bytes)


    n, k = x.shape
    eps_squared = Decimal(0); rounding_squared = Decimal(0); score_squared = Decimal(0)
    projection_bounds = []
    start = time.monotonic()
    for j in range(k):
        loads = []; score = ZERO
        for i in range(n):
            exact_r = sub(sub(point(x[i,j]), point(ax[i,j])), point(fx[i,j]))
            loads.append(mul(weights[i], exact_r))
            score = add(score, mul(exact_r, residual[i]))
            error = sub(exact_r, point(r[i,j]))
            radius = max(error[0].copy_abs(), error[1].copy_abs())
            rounding_squared = HIGH.add(rounding_squared, HIGH.multiply(
                weights[i][1], HIGH.multiply(radius, radius)))
        energy = energy_bound(topology, loads, weights)
        eps_squared = HIGH.add(eps_squared, energy)
        score_max = max(score[0].copy_abs(), score[1].copy_abs())
        score_squared = HIGH.add(score_squared, HIGH.multiply(score_max, score_max))
        projection_bounds.append(str(energy))

    w = np.array([float((a+b)/2) for a,b in weights])
    if (r.dtype != np.dtype('float64') or w.dtype != np.dtype('float64')
            or n > 2**20 or not np.isfinite(r).all() or not np.isfinite(w).all()
            or np.any(w < 2.0**-100) or np.any(w > 2.0**100)
            or np.any((r != 0) & (np.abs(r) < 2.0**-100))
            or np.any(np.abs(r) > 2.0**100)):
        raise ValueError('FP64 Gram arithmetic envelope exceeded')
    gram = r.T @ (w[:,None]*r)
    absolute_gram = np.abs(r).T @ (w[:,None]*np.abs(r))
    if not np.isfinite(gram).all() or not np.isfinite(absolute_gram).all():
        raise ValueError('nonfinite Gram')
    unit = HIGH.divide(Decimal(1), Decimal(2**53))
    gamma = HIGH.divide(HIGH.multiply(Decimal(3*n), unit),
                        LOW.subtract(Decimal(1), HIGH.multiply(Decimal(3*n), unit)))
    if gamma >= 1:
        raise ValueError('dot-product rounding envelope invalid')
    amplifier = HIGH.divide(Decimal(1), LOW.subtract(Decimal(1), gamma))
    gershgorin = []
    dot_errors = []
    trace_upper = Decimal(0)
    for j in range(k):
        off = Decimal(0); absolute_row = Decimal(0)
        for h in range(k):
            if h != j:
                off = HIGH.add(off, point(gram[j,h])[0].copy_abs())
            absolute_row = HIGH.add(absolute_row, point(absolute_gram[j,h])[0])
        gershgorin.append(LOW.subtract(point(gram[j,j])[0], off))
        dot_errors.append(HIGH.multiply(gamma, HIGH.multiply(amplifier, absolute_row)))
        trace_upper = HIGH.add(trace_upper, HIGH.multiply(amplifier, point(absolute_gram[j,j])[0]))
    relative_weight_error = Decimal(0)
    for wi, interval in zip(w, weights):
        delta = sub(interval, point(wi))
        radius = max(delta[0].copy_abs(), delta[1].copy_abs())
        relative_weight_error = max(relative_weight_error, HIGH.divide(radius, point(wi)[0]))
    weight_gram_error = HIGH.multiply(relative_weight_error, trace_upper)
    true_trace_upper = HIGH.multiply(HIGH.add(Decimal(1), relative_weight_error), trace_upper)
    sqrt_upper = lambda v: HIGH.next_plus(HIGH.sqrt(v))
    sqrt_lower = lambda v: LOW.next_minus(LOW.sqrt(v))
    rounding_gram_error = HIGH.add(rounding_squared, HIGH.multiply(Decimal(2),
        HIGH.multiply(sqrt_upper(true_trace_upper), sqrt_upper(rounding_squared))))
    curvature = min(gershgorin)
    for error in (max(dot_errors), weight_gram_error, rounding_gram_error, eps_squared):
        curvature = LOW.subtract(curvature, error)
    if curvature <= 0:
        raise ValueError('finite curvature not certified')
    profile_score = HIGH.add(sqrt_upper(score_squared),
                            HIGH.multiply(sqrt_upper(eps_squared), sqrt_upper(fe_energy)))
    newton_squared = HIGH.add(fe_energy, HIGH.divide(HIGH.multiply(profile_score, profile_score), curvature))
    radius = HIGH.divide(HIGH.multiply(Decimal(2), sqrt_upper(newton_squared)), sqrt_lower(curvature))
    condition = HIGH.divide(sqrt_upper(newton_squared), sqrt_lower(min(a for a,b in weights)))
    threshold = LOW.divide(LOW.next_minus(LOW.ln(Decimal(2))), Decimal(2))
    if condition >= threshold:
        raise ValueError('exponential curvature condition not certified')
    # The same full-model Newton ball bounds every observation's predictor.
    # Include reconstruction error: the published means need not be exactly
    # representable by the independently constructed feasible coefficients.
    public_optimum_log_error = HIGH.add(public_feasible_log_error,
                                       HIGH.multiply(Decimal(2), condition))
    report = {'campaign_admission': False, 'coefficient_accuracy_certified': radius < Decimal('1e-10'), 'precision':60,
              'fe_energy_upper':str(fe_energy), 'projection_energy_upper':projection_bounds,
              'finite_curvature_lower':str(curvature), 'newton_squared_upper':str(newton_squared),
              'coefficient_radius_upper':str(radius), 'condition_upper':str(condition),
              'condition_threshold_lower':str(threshold), 'radius_below_1e_10':radius < Decimal('1e-10'),
              'public_to_feasible_log_mean_upper':str(public_feasible_log_error),
              'public_to_optimum_log_mean_upper':str(public_optimum_log_error),
              'dot_error_upper':str(max(dot_errors)), 'weight_gram_error_upper':str(weight_gram_error),
              'residual_rounding_gram_error_upper':str(rounding_gram_error),
              'elapsed_seconds':time.monotonic()-start}

    report['modeled_workspace_bytes'] = modeled_bytes
    report['arithmetic'] = 'IEEE FP64 guarded Gram; 60-digit outward Decimal certificate'
    report['runtime'] = {'python': platform.python_version(), 'numpy': np.__version__, 'scipy': scipy.__version__}
    report['proof_source_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    report['arrays_sha256'] = {}
    for name, array in (('y',y),('x',x),('beta',beta),('public_mu',public_mu),('groups',g),
                        ('state_worker',worker_alpha),('state_firm',firm_alpha),
                        ('control_worker',ax),('control_firm',fx),
                        ('tree_parent',np.asarray(topology[2],dtype='<i8')),
                        ('tree_edge',np.asarray(topology[3],dtype='<i8'))):
        a = np.ascontiguousarray(array)
        report['arrays_sha256'][name] = {'shape': list(a.shape), 'dtype': a.dtype.str,
            'sha256': hashlib.sha256(a.tobytes()).hexdigest()}
    report['firm_multiplier'] = firm_multiplier
    report['projection_proposal'] = project.__name__
    report['components'] = sum(i == parent for i, parent in enumerate(topology[2]))
    return report
