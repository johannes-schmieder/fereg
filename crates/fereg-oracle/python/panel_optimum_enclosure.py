"""Independent residual-to-forward-error bounds for the two retained fits.

Let D contain worker/firm indicators and Z the finite regressors plus all but
one third-FE indicator. Construct e with [D,Z]'e=[D,Z]'(y-p). A spanning-forest
flow balances D; weighted within-worker contrasts on stayers balance Z without
changing D. Therefore ||P_A(y-p)|| <= ||e|| for OLS. The stayer Gram provides an
independent positive lower bound for the identified Z coordinates. For Poisson,
use ||e/sqrt(p)|| and local exponential curvature to enclose an optimum around
an independently reconstructed feasible predictor. These development-only helpers never run fereg or fixest and never change
results or comparison tolerances. They apply to the balanced five-row panels
with sixteen finite columns and a 32-level third FE used by the large companion
witnesses. See docs/within_numerical_enclosure.md for the derivation and scope.
"""

import math

BLOCK = 40960


def topology(firm, np, bridge):
    nw = len(firm)
    nf = int(firm.max()) + 1
    first = np.broadcast_to(np.arange(5, dtype=np.int8), firm.shape).copy()
    for t in range(1, 5):
        for prev in reversed(range(t)):
            equal = firm[:, t] == firm[:, prev]
            first[equal, t] = prev
    cell = (np.arange(nw, dtype=np.int64)[:, None] * 5 + first).ravel()
    mobile = np.flatnonzero(np.any(firm != firm[:, :1], axis=1))
    unique = first[mobile] == np.arange(5)[None, :]
    left = np.broadcast_to(np.arange(len(mobile))[:, None], unique.shape)[unique]
    right = firm[mobile][unique].astype(np.int64) + len(mobile)
    edge_cells = (mobile[:, None] * 5 + np.arange(5)[None, :])[unique]
    # A breadth-first forest gives much shorter paths than a DFS forest and
    # reduces the norm of this explicit, deliberately non-optimal score flow.
    from scipy.sparse import coo_matrix

    vertices = len(mobile) + nf
    ids = np.arange(len(left)) + 1
    graph = coo_matrix(
        (np.r_[ids, ids], (np.r_[left, right], np.r_[right, left])),
        shape=(vertices, vertices),
    ).tocsr()
    parent = np.full(vertices, -2, dtype=np.int64)
    pe = np.full(vertices, -1, dtype=np.int64)
    order = np.empty(vertices, dtype=np.int64)
    depth = np.zeros(vertices, dtype=np.int64)
    head = tail = 0
    for root in range(vertices):
        if parent[root] != -2:
            continue
        parent[root] = -1
        order[tail] = root
        tail += 1
        while head < tail:
            v = order[head]
            head += 1
            for pos in range(graph.indptr[v], graph.indptr[v + 1]):
                other = graph.indices[pos]
                if parent[other] != -2:
                    continue
                parent[other] = v
                pe[other] = graph.data[pos] - 1
                depth[other] = depth[v] + 1
                order[tail] = other
                tail += 1
    assert tail == vertices
    return dict(
        mobile=mobile,
        cell=cell,
        left=left,
        right=right,
        edge_cells=edge_cells,
        parent=parent,
        pe=pe,
        order=order,
        depth=depth,
        nf=nf,
    )


def exact_pair_scores(y, p, firm, np):
    """Correctly rounded fsum of signed original binary64 values, before cancellation."""
    nw = len(y) // 5
    y5 = y.reshape(nw, 5)
    p5 = p.reshape(nw, 5)
    gw = np.empty(nw)
    for j in range(nw):
        gw[j] = math.fsum(y5[j].tolist() + (-p5[j]).tolist())
    order = np.argsort(firm.ravel(), kind="stable")
    ids = firm.ravel()[order]
    cuts = np.r_[0, np.flatnonzero(ids[1:] != ids[:-1]) + 1, len(ids)]
    yy = y[order]
    pp = p[order]
    gf = np.zeros(int(ids[-1]) + 1)
    for a, b in zip(cuts[:-1], cuts[1:]):
        gf[ids[a]] = math.fsum(yy[a:b].tolist() + (-pp[a:b]).tolist())
    return gw, gf


def pair_correction(y, p, w, firm, top, np):
    ld = np.longdouble
    u = np.finfo(float).eps
    nw = len(y) // 5
    gw, gf = exact_pair_scores(y, p, firm, np)
    masses = w.reshape(nw, 5).sum(axis=1)
    e0 = (w.reshape(nw, 5) * (gw / masses)[:, None]).ravel()
    remw = gw.astype(ld) - e0.reshape(nw, 5).sum(axis=1, dtype=ld)
    remf = gf.astype(ld)
    np.add.at(remf, firm.ravel(), -e0.astype(ld))
    mob = top["mobile"]
    nm = len(mob)
    divergence = np.r_[remw[mob], -remf]
    flow = np.zeros(len(top["left"]), dtype=ld)
    for v in top["order"][::-1]:
        parent = top["parent"][v]
        if parent < 0:
            continue
        flow[top["pe"][v]] = divergence[v] * (1 if v < nm else -1)
        divergence[parent] += divergence[v]
    root_error = float(np.sum(abs(divergence[top["parent"] < 0]), dtype=ld))
    cell = top["cell"]
    cmass = np.bincount(cell, weights=w, minlength=len(y))
    cflow = np.zeros(len(y), dtype=ld)
    cflow[top["edge_cells"]] = flow
    e = e0.astype(ld) + w.astype(ld) * cflow[cell] / cmass[cell]
    norm = float(np.sqrt(np.sum(e * e / w, dtype=ld)))
    # fsum error is at most a few ulps of each small score. All subsequent
    # operations act on these small scores, never on raw canceling outcomes.
    magnitude = float(
        np.sum(abs(gw), dtype=ld)
        + np.sum(abs(gf), dtype=ld)
        + np.sum(abs(e0), dtype=ld)
        + np.sum(abs(flow), dtype=ld)
    )
    round_l1 = 128 * u * (1 + int(top["depth"].max(initial=0))) * magnitude
    minimum_mass = float(np.min(cmass[cmass > 0]))
    edges = len(flow)
    round_norm = (round_l1 + root_error) * math.sqrt(max(1, edges) / minimum_mass)
    checkw = gw.astype(ld) - e.reshape(nw, 5).sum(axis=1, dtype=ld)
    checkf = gf.astype(ld)
    np.add.at(checkf, firm.ravel(), -e)
    discrepancy = float(np.sum(abs(checkw), dtype=ld) + np.sum(abs(checkf), dtype=ld))
    assert discrepancy <= 4 * (round_l1 + root_error) + 1e-28
    return e, dict(
        norm=norm,
        round_norm=round_norm,
        score_round_l1=round_l1,
        root_score_l1=root_error,
        moment_discrepancy_l1=discrepancy,
        tree_max_depth=int(top["depth"].max(initial=0)),
        minimum_cell_mass=minimum_mass,
    )


def design_block(data, start, stop, np):
    codes = data[20, start:stop].astype(np.int64) - 1
    assert (codes >= 0).all() and (codes < 32).all()
    z = np.zeros((stop - start, 47))
    z[:, :16] = data[2:18, start:stop].T
    take = codes < 31
    z[np.flatnonzero(take), 16 + codes[take]] = 1
    return z


def gram_and_score(data, y, p, w, e, firm, beta, np):
    n = len(y)
    ld = np.longdouble
    G = np.zeros((47, 47))
    h = np.zeros(31, dtype=ld)
    leaves = np.empty(((n + 127) // 128, 47), dtype=ld)
    leaf_index = 0
    subtraction_round = np.zeros(47, dtype=ld)
    all_energy = ld(0)
    score_absolute = np.zeros(47, dtype=ld)
    raw_energy = ld(0)
    for start in range(0, n, BLOCK):
        stop = min(n, start + BLOCK)
        z = design_block(data, start, stop, np)
        weights = w[start:stop]
        residual = y[start:stop].astype(ld) - p[start:stop].astype(ld) - e[start:stop]
        # Short long-double leaves give a conservative explicit summation bound.
        for a in range(0, len(z), 128):
            product = z[a : a + 128].astype(ld) * residual[a : a + 128, None]
            leaves[leaf_index] = np.sum(product, axis=0, dtype=ld)
            leaf_index += 1
            score_absolute += np.sum(abs(product), axis=0, dtype=ld)
        raw_scale = (
            abs(y[start:stop].astype(ld))
            + abs(p[start:stop].astype(ld))
            + abs(e[start:stop])
        )
        subtraction_round += np.sum(
            abs(z.astype(ld)) * raw_scale[:, None], axis=0, dtype=ld
        )
        all_energy += np.sum((weights[:, None] * z * z).astype(ld), dtype=ld)
        z5 = z.reshape(-1, 5, 47)
        w5 = weights.reshape(-1, 5)
        mass = w5.sum(axis=1)
        stay = np.all(
            firm[start // 5 : stop // 5] == firm[start // 5 : stop // 5, :1], axis=1
        )
        centered = (
            z5 - (np.sum(w5[:, :, None] * z5, axis=1) / mass[:, None])[:, None, :]
        )
        zs = centered[stay].reshape(-1, 47)
        ws = w5[stay].ravel()
        G += zs.T @ (ws[:, None] * zs)
        raw_energy += np.sum((w5[stay, :, None] * z5[stay] ** 2).astype(ld), dtype=ld)
        # Recover third-effect coefficients independently from stayer differences.
        eta = data[26 if data.shape[0] == 28 else 25, start:stop].astype(ld)
        absorbed = eta - z[:, :16].astype(ld) @ beta.astype(ld)
        h += np.sum(
            zs[:, 16:].astype(ld)
            * (ws.astype(ld) * absorbed.reshape(-1, 5)[stay].ravel())[:, None],
            axis=0,
            dtype=ld,
        )
    assert leaf_index == len(leaves)
    levels = 0
    while len(leaves) > 1:
        pairs = len(leaves) // 2
        combined = leaves[: 2 * pairs : 2] + leaves[1 : 2 * pairs : 2]
        leaves = (
            np.concatenate((combined, leaves[-1:])) if len(leaves) % 2 else combined
        )
        levels += 1
    g = leaves[0]
    eigen = np.linalg.eigvalsh(G)
    ops = 100 * n + 10000
    u = np.finfo(float).eps
    gamma = ops * u / (1 - ops * u)
    formation = float(gamma * raw_energy)
    # Verify an inverse-residual bound rather than assume an eigensolver's
    # backward error constant. B is just a proposed inverse; its residual is
    # recomputed in extended precision and explicitly bounded.
    B = np.linalg.inv(G)
    product = B.astype(ld) @ G.astype(ld)
    residual = np.eye(47, dtype=ld) - product
    product_error = 128 * np.finfo(ld).eps * (abs(B).astype(ld) @ abs(G).astype(ld))
    inverse_residual = float(
        np.sqrt(np.sum((abs(residual) + product_error) ** 2, dtype=ld))
    ) * (1 + 256 * u)
    inverse_norm = math.sqrt(
        float(np.max(np.sum(abs(B).astype(ld), axis=0)))
        * float(np.max(np.sum(abs(B).astype(ld), axis=1)))
    ) * (1 + 256 * u)
    assert inverse_residual < 1
    matrix_lower = (1 - inverse_residual) / inverse_norm
    lower = matrix_lower - formation
    assert lower > 0
    spectral = 0.0
    # A certified eigenvalue lower bound permits residual-based bounds even
    # though the small dense solve itself is ordinary binary64 LAPACK.
    a = np.linalg.solve(G, np.asarray(g, dtype=float))
    solve_residual = g - G.astype(ld) @ a.astype(ld)
    score_round = (np.finfo(ld).eps * (160 + levels)) * np.asarray(
        score_absolute, dtype=float
    ) + 8 * np.finfo(ld).eps * np.asarray(subtraction_round, dtype=float)
    error = float(
        np.linalg.norm(np.asarray(solve_residual, dtype=float))
        + formation * np.linalg.norm(a)
        + np.linalg.norm(score_round)
    )
    energy = max(0.0, float(a @ G @ a) + formation * float(a @ a))
    norm = math.sqrt(energy) + error / math.sqrt(lower)
    third = np.linalg.solve(G[16:, 16:], np.asarray(h, dtype=float))
    return third, dict(
        minimum_eigenvalue=float(eigen[0]),
        formation_bound=formation,
        eigensolver_bound=spectral,
        certified_minimum_eigenvalue=lower,
        norm=norm,
        inverse_residual_bound=inverse_residual,
        inverse_norm_bound=inverse_norm,
        all_design_energy_bound=float(all_energy) * (1 + gamma),
        score_reduction_levels=levels,
        score_round_norm=float(np.linalg.norm(score_round)),
        global_score_norm=float(np.linalg.norm(np.asarray(g, dtype=float))),
        solve_residual_norm=float(
            np.linalg.norm(np.asarray(solve_residual, dtype=float))
        ),
        stayer_rows=int(np.count_nonzero(np.all(firm == firm[:, :1], axis=1))) * 5,
    )


def feasibility(data, beta, third, firm, top, w, np):
    n = data.shape[1]
    ld = np.longdouble
    eta = data[26 if data.shape[0] == 28 else 25]
    remainder = np.empty(n, dtype=ld)
    arithmetic_scale = np.empty(n, dtype=ld)
    third_all = np.r_[third, 0.0].astype(ld)
    for start in range(0, n, BLOCK):
        stop = min(n, start + BLOCK)
        X = data[2:18, start:stop].T.astype(ld)
        third_value = third_all[data[20, start:stop].astype(np.int64) - 1]
        remainder[start:stop] = (
            eta[start:stop].astype(ld) - X @ beta.astype(ld) - third_value
        )
        arithmetic_scale[start:stop] = (
            abs(eta[start:stop].astype(ld))
            + abs(X) @ abs(beta.astype(ld))
            + abs(third_value)
        )
    sums = np.zeros(n, dtype=ld)
    np.add.at(sums, top["cell"], remainder)
    counts = np.bincount(top["cell"], minlength=n)
    edge = sums[top["edge_cells"]] / counts[top["edge_cells"]]
    nm = len(top["mobile"])
    potential = np.zeros(nm + top["nf"], dtype=ld)
    for v in top["order"]:
        parent = top["parent"][v]
        if parent >= 0:
            potential[v] = edge[top["pe"][v]] - potential[parent]
    firms = potential[nm:]
    workers = (remainder.reshape(-1, 5) - firms[firm]).mean(axis=1)
    delta = remainder - workers.repeat(5) - firms[firm.ravel()]
    # Chosen recovered coefficients define an explicit feasible state. Dot and
    # addition rounding in checking its distance is bounded separately.
    bound = (
        64
        * np.finfo(ld).eps
        * (1 + arithmetic_scale + abs(workers.repeat(5)) + abs(firms[firm.ravel()]))
    )
    distance = abs(delta) + bound
    return distance, dict(
        maximum_predictor_distance=float(distance.max()),
        weighted_l2_distance=float(
            np.sqrt(np.sum(w.astype(ld) * distance**2, dtype=ld))
        ),
    )


def sharpen_poisson_summary(scan, fitted_rtol=1e-7):
    """Tighten an already admitted Poisson ball without rescanning any data.

    The original radius R0=2 exp(d)(E+D) proves the optimum is inside the ball
    if r=R0/sqrt(minimum mean)<log(2). On that ball the Hessian is at least
    exp(-d-r) W. Thus its distance is at most R1=R0 exp(r)/2. Convexity of
    expm1 bounds every new row envelope by q times its old envelope plus
    (1-q) p expm1(distance), where q=R1/R0. The latter term divided by the
    tolerance is bounded by D/(sqrt(minimum mean)*fitted_rtol).
    """
    optimum = scan["optimum"]
    if optimum["kind"] != "Poisson local curvature" or not optimum["admitted"]:
        raise ValueError("an independently admitted Poisson ball is required")
    radius = optimum["weighted_radius"]
    relative_radius = optimum["maximum_radius_over_sqrt_mean"]
    if not 0 < relative_radius < math.log(2) or radius <= 0 or fitted_rtol <= 0:
        raise ValueError("invalid Poisson enclosure")
    factor = 0.5 * math.exp(relative_radius)
    assert 0 < factor < 1
    distance_allowance = (
        optimum["feasible_mean_distance"] * relative_radius / radius / fitted_rtol
    )
    # Small outward allowance for this scalar postprocessing, independent of
    # the much larger upstream allowances for the data reductions.
    outward = 1 + 128 * math.ulp(1.0)
    ratio = (
        factor * scan["maximum_tolerance_ratio"] + (1 - factor) * distance_allowance
    ) * outward
    failed_ratio = (
        factor * scan["maximum_failed_tolerance_ratio"]
        + (1 - factor) * distance_allowance
    ) * outward
    coefficient = factor * scan["finite_coefficient_absolute_error_bound"] * outward
    return dict(
        method="curvature inside the independently admitted ball",
        radius_factor=factor,
        weighted_radius=radius * factor * outward,
        maximum_tolerance_ratio_bound=ratio,
        maximum_failed_tolerance_ratio_bound=failed_ratio,
        finite_coefficient_absolute_error_bound=coefficient,
        finite_coefficients_enclosed=coefficient
        <= scan["finite_coefficient_minimum_tolerance"],
        all_predictions_enclosed=ratio <= 1,
        original_failed_rows_enclosed=(
            scan["original_failed_rows"] if failed_ratio <= 1 else None
        ),
    )
