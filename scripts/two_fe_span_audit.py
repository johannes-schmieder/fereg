"""Independent, bounded two-FE feasibility audit for saved predictions.

A spanning forest produces a witness, not a least-squares solution. Only fresh
observation and normal residuals can accept it. A failed witness falls back to
grounded, degree-scaled SciPy LSMR; neither failure proves infeasibility.
No native estimator or native projection code is used.
"""

import numpy as np
from scipy import sparse
from scipy.sparse.csgraph import breadth_first_order, connected_components
from scipy.sparse.linalg import lsmr


def _metrics(target, residual, left, right, vertices):
    scale = max(1.0, float(np.linalg.norm(target)))
    counts = np.bincount(left, minlength=vertices) + np.bincount(right, minlength=vertices)
    moments = (np.bincount(left, weights=residual, minlength=vertices)
               + np.bincount(right, weights=residual, minlength=vertices))
    if (not np.isfinite(scale) or not np.isfinite(residual).all()
            or not np.isfinite(moments).all()):
        raise ValueError("nonfinite independent span certificate")
    relative = float(np.linalg.norm(residual) / scale)
    normal = float(np.max(np.abs(moments) / np.sqrt(counts)) / scale)
    return relative, normal


def audit_two_fe_span(target, groups, *, memory_bytes=512 * 1024**2,
                      max_iterations=5000):
    target, groups = np.asarray(target), np.asarray(groups)
    if (target.dtype.kind not in 'iuf' or groups.dtype.kind not in 'iuf'
            or target.ndim != 1 or target.size < 2 or groups.shape != (target.size, 2)
            or not np.isfinite(target).all() or not np.isfinite(groups).all()
            or np.any(groups != np.floor(groups))):
        raise ValueError("invalid two-FE span inputs")
    if type(max_iterations) is not int or max_iterations < 1:
        raise ValueError("invalid projection iteration budget")
    n = target.size
    # Includes input conversions, sorting, duplicate CSR assembly, virtual-root
    # traversal, pointer doubling, and the fallback's simultaneous sparse banks.
    # The worst case has two vertices per observation. Admit before materializing
    # any topology or observation-sized scratch; caller-owned inputs are separate.
    modeled_bytes = 1280 * n
    if memory_bytes < modeled_bytes:
        raise MemoryError("independent two-FE scratch exceeds audit budget")
    target = target.astype(np.float64, copy=False)
    _, left = np.unique(groups[:, 0], return_inverse=True)
    _, right = np.unique(groups[:, 1], return_inverse=True)
    right += int(left.max()) + 1
    vertices = int(right.max()) + 1
    graph = sparse.coo_matrix((np.ones(2*n),
                               (np.concatenate((left, right)),
                                np.concatenate((right, left)))),
                              shape=(vertices, vertices)).tocsr()
    components, labels = connected_components(graph, directed=False)
    roots = np.full(components, vertices, dtype=np.int64)
    np.minimum.at(roots, labels, np.arange(vertices))
    # One virtual root makes a single traversal cover disconnected graphs.
    # Calling SciPy BFS separately for every component would repeatedly allocate
    # vertex-sized traversal state on singleton-heavy graphs.
    graph.resize((vertices + 1, vertices + 1))
    graph = sparse.vstack((graph[:vertices], sparse.csr_matrix(
        (np.ones(components), (np.zeros(components, dtype=np.int64), roots)),
        shape=(1, vertices + 1))), format="csr")
    order, parent = breadth_first_order(graph, vertices, directed=True)
    if order.size != vertices + 1:
        raise ValueError("incomplete independent spanning forest")
    del graph, order, labels
    parent[vertices] = vertices
    # Parallel observations of a tree edge share its mean target. The fresh
    # all-row residual still checks each original observation independently.
    left_child = parent[left] == right
    right_child = parent[right] == left
    child = np.concatenate((left[left_child], right[right_child]))
    delta = np.bincount(child, weights=np.concatenate(
        (target[left_child], -target[right_child])), minlength=vertices+1)
    count = np.bincount(child, minlength=vertices+1)
    np.divide(delta, count, out=delta, where=count > 0)
    del child, count, left_child, right_child
    # Signed potentials satisfy target = p(left)-p(right). Pointer doubling
    # sums tree paths in compiled array operations rather than Python row loops.
    ancestor = parent.copy()
    jumps = 0
    while np.any(ancestor != vertices):
        delta += delta[ancestor]
        ancestor = ancestor[ancestor]
        jumps += 1
        if jumps > vertices.bit_length():
            raise ValueError("invalid independent forest ancestry")
    del parent, ancestor
    residual = target - (delta[left] - delta[right])
    relative, normal = _metrics(target, residual, left, right, vertices)
    forest = dict(fresh_residual_relative=relative, fresh_normal_max=normal)
    passed = bool(relative <= 1e-10 and normal <= 1e-12)
    result = dict(method="two_fe_forest", components=int(components),
                  forest_pointer_rounds=jumps, forest=forest,
                  recurrence_stop=None, iterations=0, recurrence_converged=None,
                  fresh_residual_relative=relative, fresh_normal_max=normal,
                  modeled_scratch_bytes=modeled_bytes,
                  coefficient_binding_pass=passed)
    if passed:
        return result
    del residual, delta
    indices = np.column_stack((left, right)).ravel()
    design = sparse.csr_matrix((np.ones(2*n), indices, np.arange(n+1)*2),
                               shape=(n, vertices))
    keep = np.ones(vertices, dtype=bool)
    keep[roots] = False
    grounded = design[:, keep]
    degree = np.asarray(grounded.sum(axis=0)).ravel()
    inverse_norm = 1.0 / np.sqrt(degree)
    scaled = grounded.multiply(inverse_norm).tocsr()
    solution = lsmr(scaled, target, atol=1e-13, btol=1e-13,
                    conlim=1e14, maxiter=max_iterations)
    # Check the unscaled original indicator operator, including grounded rows.
    # A small recurrence residual for the scaled operator is not a certificate.
    residual = target - grounded @ (inverse_norm * solution[0])
    relative, normal = _metrics(target, residual, left, right, vertices)
    converged = int(solution[1]) in (0, 1, 2, 4, 5)
    result.update(method="two_fe_grounded_scaled_lsmr",
                  recurrence_stop=int(solution[1]), iterations=int(solution[2]),
                  recurrence_converged=converged,
                  fresh_residual_relative=relative, fresh_normal_max=normal,
                  coefficient_binding_pass=bool(converged and relative <= 1e-10
                                                and normal <= 1e-12))
    return result
