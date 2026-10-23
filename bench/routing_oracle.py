#!/usr/bin/env python3
"""Independent, bounded categorical OLS references for routing inputs.

The largest FE block is eliminated exactly. The remaining grounded Schur system
is factored directly with SciPy LAPACK, independently of every fereg solver.
Known per-component constant gauges give a rank upper bound. Grounded spectra
must clear conservative formation and projection error bounds under the usual
floating error model; this is sufficient numerical rank admission, not interval
certification. Extra aliases or ambiguous spectra are refused, never repaired
by dropping columns.

Canonical numeric inputs only: positive pw or unit weights, plain continuous X,
categorical intercept FEs, HC1 or unit-weight IID/one-way CR1, pairwise DoF,
optional first-FE cluster nesting for two FEs, and no saved effects. Original
rows are preserved, including independently requested singleton pruning. The
CLI writes separate input-bound JSON/NPZ references.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import zipfile

import numpy as np
from native_io import bounded_line, case_header, digest, numbers

KIND = 'routing_categorical_oracle'
MAX_ROWS, MAX_RETAINED, MAX_X, MAX_FE = 600_000, 2048, 32, 16
MAX_BYTES, MAX_SCHUR_WORK = 1 << 30, 2_000_000_000
EXPANDED_MEMORY_BYTES = 2 << 30
MAX_WEIGHT_RATIO = 1e12
PARSER_SOURCE = Path(__file__).with_name('native_io.py')
MOMENT_LIMIT = 1e-10
OUTPUTS = ('sample', 'residual', 'drop_reason', 'xb', 'xbd', 'absorbed')


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def memory_budget(value):
    """Explicit reference-generation allowance; never a numerical tolerance."""
    _require(type(value) is int and value in {MAX_BYTES, EXPANDED_MEMORY_BYTES},
             'oracle memory budget must be the default one GiB or declared two GiB')
    return value


def _matrix(stream, count):
    text = stream.read(count * 17)
    _require(len(text) == count * 17 and all(text[i] == 10 for i in range(16, len(text), 17)),
             'invalid fixed-width numeric replay data')
    return np.frombuffer(bytes.fromhex(text.decode('ascii')), dtype='>f8').astype(float)


def read_case(path):
    """Validate the actual frozen canonical bindings, not a generator filename."""
    with Path(path).open('rb') as stream:
        shape, arguments = case_header(stream)
        pairs = [arg.decode().split('=', 1) for arg in arguments[1:]]
        fields = dict(pairs)
        n, width, _, matrix_count, macros = shape
        k, q = int(fields['term_count']), int(fields['absorb_term_count'])
        _require(arguments[0] == b'estimate-ols' and len(fields) == len(pairs)
                 and 1 <= n <= MAX_ROWS and 1 <= k <= MAX_X and 2 <= q <= MAX_FE
                 and width <= 64 and macros == 0, 'unsupported oracle input dimensions/model')
        vce = fields.get('vce')
        cluster_count = '1' if vce == 'cluster' else '0'
        _require(vce in {'robust', 'iid', 'cluster'} and all(fields.get(key) == value for key, value in {
            'dof': 'pairwise',
            'cluster_term_count': cluster_count, 'cluster_component_count': cluster_count,
            'model_major': '2', 'model_minor': '1',
        }.items()), 'oracle requires canonical HC1/unit IID/one-way CR1')
        nesting = fields.get('account_cluster_nesting')
        _require(nesting == '0' or nesting == '1' and vce == 'cluster' and q == 2,
                 'nesting oracle supports only canonical first-FE two-way clustering')
        _require(fields.get('keep_singletons') in {'0', '1'}
                 and fields.get('include_intercept') in {'0', '1'}, 'invalid singleton/intercept request')
        _require(fields.get('output_saved_channels') == '|'.join(['1:0'] * q),
                 'saved-effect fixtures require their separate independent witness')
        start = stream.tell()
        inputs = int(fields['output_sample']) - 1
        _require(inputs in {2 + k + q, 3 + k + q} and width == inputs + 6,
                 'noncanonical input/output columns')
        data = []
        for j in range(width):
            _require(bounded_line(stream) == b'0\n', 'oracle requires numeric replay columns')
            if j < inputs:
                data.append(_matrix(stream, n))
            else:
                stream.seek(n * 17, 1)
        tables = {}
        for _ in range(matrix_count):
            length = int(bounded_line(stream))
            _require(0 < length <= 1000, 'invalid matrix name length')
            name = stream.read(length).decode()
            _require(stream.read(1) == b'\n' and name not in tables, 'invalid/duplicate matrix name')
            rows, columns = map(int, bounded_line(stream).split())
            _require(0 < rows * columns <= 100_000, 'oracle matrix metadata bound exceeded')
            tables[name] = _matrix(stream, rows * columns).reshape(rows, columns)
        _require(not stream.read(1), 'unexpected replay trailing bytes')
        stream.seek(start)
        body = hashlib.sha256()
        for block in iter(lambda: stream.read(1 << 20), b''):
            body.update(block)
    expected = {
        'source_matrix': [[j, 0] for j in range(1, k + q + 1)],
        'term_matrix': [[j, j, j, 1, 1, 0, 0, 0] for j in range(1, k + 1)],
        'component_matrix': [[j, 1, j, 1, 1, 0, 0, 2, 1, j, j, 0, 0] for j in range(1, k + 1)],
        'absorb_term_matrix': [[j, 1, 1, 0] for j in range(1, q + 1)],
        'absorb_component_matrix': [[j, 1, 1, k + j] for j in range(1, q + 1)],
        'binding_matrix': [[1, 1, 0], [2, 2, 0], *[[j + 2, 3, j] for j in range(1, k + q + 1)],
                           *([[inputs, 6, 0]] if inputs == 3 + k + q else [])],
    }
    if vce == 'cluster':
        expected.update(cluster_term_matrix=[[1, 1]], cluster_component_matrix=[[1, 1, k+1]])
    _require(all(np.array_equal(tables.get(fields.get(name)), value) for name, value in expected.items()),
             'canonical model/source binding differs from oracle contract')
    _require(all(np.all(np.isfinite(v) & (np.abs(v) < 8e307)) for v in data)
             and np.all(data[0] == 1), 'oracle does not infer missing-data or candidate selection')
    outputs = {name: int(fields['output_' + name]) - 1 for name in OUTPUTS}
    _require(list(outputs.values()) == list(range(inputs, width)), 'unexpected output registry')
    w = data[-1] if inputs == 3 + k + q else np.ones(n)
    _require(np.all(w > 0), 'oracle requires positive weights')
    _require(vce == 'robust' or inputs == 2+k+q, 'IID/cluster oracle requires unit weights')
    return dict(y=data[1], x=np.column_stack(data[2:2+k]), factors=data[2+k:2+k+q], weights=w,
                vce=vce, clusters=data[2+k] if vce == 'cluster' else None,
                account_cluster_nesting=nesting == '1',
                include_intercept=fields['include_intercept'] == '1',
                keep_singletons=fields['keep_singletons'] == '1', rows=n, columns=width,
                outputs=outputs, case_sha256=digest(path), body_sha256=body.hexdigest())


def _sample(factors, keep_singletons):
    codes = [np.unique(f, return_inverse=True)[1] for f in factors]
    active = np.ones(len(codes[0]), dtype=bool)
    if not keep_singletons:
        for _ in range(1000):
            remove = np.zeros(len(active), dtype=bool)
            for code in codes:
                counts = np.bincount(code[active], minlength=int(code.max()) + 1)
                remove |= active & (counts[code] == 1)
            if not remove.any():
                break
            active[remove] = False
        else:
            raise ValueError('independent singleton fixed point exceeded 1000 rounds')
    _require(active.any(), 'empty retained sample')
    return active, [np.unique(code[active], return_inverse=True)[1] for code in codes]


def _gamma(operations):
    """Conservative standard floating summation/product bound (eps >= unit roundoff)."""
    product = operations * np.finfo(float).eps
    _require(0 <= product < 1, 'invalid floating error-bound operation count')
    return product / (1-product)


def _spectrum(matrix, label, absolute_error=None):
    from scipy.linalg import eigh, cho_factor
    _require(np.all(np.isfinite(matrix)), label + ': nonfinite matrix')
    diagonal = np.diag(matrix)
    _require(np.all(diagonal > 0), label + ': nonpositive diagonal / additional alias')
    scale = 1 / np.sqrt(diagonal)
    scaled = matrix * scale[:, None] * scale[None, :]
    _require(np.all(np.isfinite(scaled)), label + ': nonfinite equilibration')
    values = eigh(scaled, eigvals_only=True, check_finite=False)
    eigen_margin = 256 * np.finfo(float).eps * len(values) * values[-1]
    formation = 0.
    if absolute_error is not None:
        _require(np.all(np.isfinite(absolute_error)) and np.all(absolute_error >= 0),
                 label + ': nonfinite/negative formation-error bound')
        formation = float(np.linalg.norm(absolute_error * scale[:,None] * scale[None,:], np.inf))
    threshold = eigen_margin + formation
    _require(np.all(np.isfinite(values)) and np.isfinite(threshold) and values[0] > threshold,
             label + ': ambiguous rank or conditioning')
    return scale, cho_factor(scaled, lower=True, check_finite=False), dict(
        minimum_eigenvalue=float(values[0]), maximum_eigenvalue=float(values[-1]),
        eigensolver_margin=float(eigen_margin), formation_error_bound=formation,
        required_margin=float(threshold), condition=float(values[-1] / values[0]))


def _moments(codes, values, weights, original):
    _require(np.all(np.isfinite(values)), 'nonfinite original-design residual values')
    norms = np.sqrt(np.sum(weights[:, None] * original**2, axis=0))
    _require(np.all(np.isfinite(norms)), 'nonfinite original-design norm')
    maximum = 0.
    for code in codes:
        degree = np.bincount(code, weights=weights)
        for j in range(values.shape[1]):
            moment = np.bincount(code, weights=weights * values[:, j])
            scale = np.sqrt(degree) * max(norms[j], np.finfo(float).tiny)
            _require(np.all(np.isfinite(moment)) and np.all(np.isfinite(scale)) and np.all(scale > 0),
                     'nonfinite or nonpositive original-design moment normalization')
            ratio = np.abs(moment) / scale
            _require(np.all(np.isfinite(ratio)), 'nonfinite original-design moment ratio')
            maximum = max(maximum, float(np.max(ratio)))
    return maximum


def _pair_redundancies(codes, levels):
    from scipy import sparse
    from scipy.sparse.csgraph import connected_components
    redundant = [0] * len(codes)
    for later in range(1, len(codes)):
        for earlier in range(later):
            n = levels[earlier] + levels[later]
            rows, cols = codes[earlier], codes[later] + levels[earlier]
            graph = sparse.coo_matrix((np.ones(len(rows)), (rows, cols)), shape=(n, n)).tocsr()
            redundant[later] = max(redundant[later], int(connected_components(graph, directed=False, return_labels=False)))
    return redundant


def _projection_errors(e, r, keep, degree, reduced, spectrum, original, projected, fe_left, fe_right, w):
    """Bound weighted FWL column errors against the original design, for every X combination.

    The anchored FE Gram H=T' diag(D,S) T has a positive lower eigenvalue
    bound. Each row of D^-1 C is a sub-probability vector in each retained
    partition, so ||D^-1 C||2 <= sqrt(ne*(q-1)) without rounding C or D.
    The FE-span error is bounded by ||F'W projected||/sqrt(lambda_min(H));
    reconstruction rounding bounds the complementary projection error.
    """
    m, ne = e.shape
    q_minus_one = int(r.indptr[1] - r.indptr[0])
    rounding = _gamma(8*m+32)
    d_lower = degree.min() / (1+rounding)
    s_lower = (spectrum['minimum_eigenvalue'] - spectrum['required_margin']) * np.diag(reduced).min()
    s_lower /= 1+_gamma(32)
    h_lower = min(d_lower, s_lower) / ((1+np.sqrt(ne*q_minus_one))**2 * (1+_gamma(32)))
    _require(np.isfinite(h_lower) and h_lower > 0, 'invalid anchored FE rank lower bound')
    weighted = w[:,None] * projected
    magnitude = w[:,None] * np.abs(projected)
    moments = np.vstack([np.abs(e.T @ weighted), np.abs((r.T @ weighted)[keep])])
    moment_error = rounding * np.vstack([e.T @ magnitude, (r.T @ magnitude)[keep]])
    # Factor two safely dominates the norm and positive-sum evaluation errors.
    span_error = 2*np.linalg.norm(moments+moment_error, axis=0) / np.sqrt(h_lower)
    reconstruction = _gamma(q_minus_one+5) * (np.abs(original) + np.abs(e @ fe_left) + r @ np.abs(fe_right))
    reconstruction_error = 2*np.sqrt(np.sum(w[:,None]*reconstruction**2, axis=0))
    errors = span_error + reconstruction_error
    _require(np.all(np.isfinite(errors)), 'nonfinite original-design projection error bound')
    return errors, float(h_lower)


def solve(y, x, factors, weights=None, *, keep_singletons=False, include_intercept=True,
          vce='robust', clusters=None, account_cluster_nesting=False,
          memory_budget_bytes=MAX_BYTES):
    """Direct FWL projection and independent HC1/unit IID/one-way CR1 evidence."""
    from scipy import sparse
    from scipy.linalg import cho_solve
    from scipy.sparse.csgraph import connected_components
    memory_budget(memory_budget_bytes)
    y, x = np.asarray(y, dtype=float), np.asarray(x, dtype=float)
    n, k = x.shape
    q = len(factors)
    _require(y.shape == (n,) and 2 <= q <= MAX_FE and 0 < n <= MAX_ROWS and 0 < k <= MAX_X
             and all(np.asarray(f).shape == (n,) for f in factors), 'oracle dimension limit')
    w = np.ones(n) if weights is None else np.asarray(weights, dtype=float)
    _require(w.shape == (n,) and np.all(np.isfinite(w)) and np.all(w > 0)
             and np.all(np.isfinite(x)) and np.all(np.isfinite(y))
             and all(np.all(np.isfinite(f)) for f in factors), 'oracle requires finite positive WLS data')
    _require(vce in {'robust', 'iid', 'cluster'} and (vce == 'robust' or np.all(w == 1)),
             'unsupported VCE or nonunit IID/cluster weights')
    _require((vce == 'cluster') == (clusters is not None), 'cluster codes/VCE mismatch')
    if clusters is not None:
        clusters = np.asarray(clusters)
        _require(clusters.shape == (n,) and np.all(np.isfinite(clusters)), 'invalid cluster codes')
    _require(type(account_cluster_nesting) is bool and (not account_cluster_nesting
             or vce == 'cluster' and q == 2 and np.array_equal(clusters, factors[0])),
             'nesting oracle supports only canonical first-FE two-way clustering')
    active, codes = _sample(factors, keep_singletons)
    levels = [int(c.max()) + 1 for c in codes]
    eliminated = int(np.argmax(levels))
    ne, nr = levels[eliminated], sum(levels) - levels[eliminated]
    modeled = n * (128 + 256*k + 96*q) + 96*nr*nr + 64*ne*(k+q+2)
    _require(nr <= MAX_RETAINED and sum(levels) <= 50_000 and modeled <= memory_budget_bytes,
             'oracle Schur dimension or declared modeled memory bound exceeded')
    y, x, w = y[active], x[active], w[active]
    m = len(y)
    _require(w.max() / w.min() <= MAX_WEIGHT_RATIO, 'oracle weight ratio bound exceeded')
    w = w / w.max()
    w *= m / w.sum()
    _require(np.all(w > 0) and np.all(np.isfinite(w)), 'weight normalization underflow/overflow')
    others = [j for j in range(q) if j != eliminated]
    offsets = np.cumsum([0] + [levels[j] for j in others])
    columns = np.column_stack([codes[j] + offsets[i] for i, j in enumerate(others)]).ravel()
    r = sparse.csr_matrix((np.ones(len(columns)), columns, np.arange(m+1)*(q-1)), shape=(m, nr))
    e = sparse.csr_matrix((np.ones(m), codes[eliminated], np.arange(m+1)), shape=(m, ne))
    wr = r.multiply(w[:, None]).tocsr()
    cross = (e.T @ wr).tocsr()
    degree = np.bincount(codes[eliminated], weights=w)
    work = int(np.sum(np.diff(cross.indptr).astype(np.int64)**2))
    _require(work <= MAX_SCHUR_WORK, 'oracle sparse Schur-product work bound exceeded')
    # Every level in a connected component shares q-1 exact constant gauges.
    adjacency = sparse.bmat([[None, cross], [cross.T, None]], format='csr')
    components, labels = connected_components(adjacency, directed=False)
    grounds = []
    for i in range(q-1):
        group = labels[ne + offsets[i]:ne + offsets[i+1]]
        unique, first = np.unique(group, return_index=True)
        _require(len(unique) == components, 'component lacks a declared FE partition')
        grounds.extend((first + offsets[i]).tolist())
    keep = np.setdiff1d(np.arange(nr), grounds)
    _require(len(keep) > 0, 'empty grounded reduced system needs a separate witness')
    positive = r.T @ wr
    negative = cross.T @ cross.multiply((1/degree)[:, None])
    schur = (positive - negative).toarray()
    reduced = schur[np.ix_(keep, keep)]
    # Both terms are nonnegative. This absolute formation bound includes
    # degree/cross sums, division, sparse products, and final subtraction.
    schur_error = _gamma(8*m+32) * (positive + negative).toarray()[np.ix_(keep, keep)]
    scale, factor, spectrum = _spectrum(reduced, 'grounded FE Schur', schur_error)
    del positive, negative, schur, schur_error
    z = np.column_stack([y, x])
    wz = w[:, None] * z
    left_rhs = e.T @ wz
    right_rhs = r.T @ wz - cross.T @ (left_rhs / degree[:, None])
    solution = scale[:, None] * cho_solve(factor, scale[:, None] * right_rhs[keep], check_finite=False)
    for _ in range(2):
        correction = right_rhs[keep] - reduced @ solution
        solution += scale[:, None] * cho_solve(factor, scale[:, None] * correction, check_finite=False)
    fe_right = np.zeros((nr, k+1))
    fe_right[keep] = solution
    fe_left = (left_rhs - cross @ fe_right) / degree[:, None]
    projected = z - fe_left[codes[eliminated]] - r @ fe_right
    projection_moment = _moments(codes, projected, w, z)
    _require(projection_moment <= MOMENT_LIMIT, 'original-design FE projection moments failed')
    ry, rx = projected[:, 0], projected[:, 1:]
    gram = rx.T @ (w[:, None] * rx)
    errors, h_lower = _projection_errors(e, r, keep, degree, reduced, spectrum,
                                         z, projected, fe_left, fe_right, w)
    x_errors = errors[1:]
    # Upper column norms and the perturbation bound hold for every linear
    # combination, including regressors or combinations exactly absorbed by F.
    x_norms = np.sqrt(np.diag(gram) / (1-_gamma(8*m+32)))
    gram_error = (np.outer(x_norms,x_errors) + np.outer(x_errors,x_norms)
                  + np.outer(x_errors,x_errors)
                  + _gamma(8*m+32)*(np.abs(rx).T @ (w[:,None]*np.abs(rx))))
    xs, xf, xspectrum = _spectrum(gram, 'within finite design', gram_error)
    bread = xs[:, None] * cho_solve(xf, np.diag(xs), check_finite=False)
    inverse_error = float(np.max(np.abs(gram @ bread - np.eye(k))))
    _require(inverse_error <= MOMENT_LIMIT, 'independent within bread inverse failed')
    beta = bread @ (rx.T @ (w * ry))
    residual = ry - rx @ beta
    fitted = y - residual
    meanx, meany = np.average(x, axis=0, weights=w), np.average(y, weights=w)
    reported = np.r_[beta, meany - meanx @ beta] if include_intercept else beta
    influence = rx @ bread
    if include_intercept:
        influence = np.column_stack([influence, 1/m - influence @ meanx])
    redundant = _pair_redundancies(codes, levels)
    actual_rank = sum(levels) - len(grounds)
    nested_categories = 0
    if account_cluster_nesting:
        # SPEC 9/pinned policy: clustered FE categories cost no absorbed DoF.
        # The other FE is represented by L2-1 centered contrasts. This policy
        # count is deliberately distinct from the numerical full-design rank
        # on a disconnected graph; estimation above still uses every dummy.
        # Fully nested second partitions have an empty grounded system and
        # remain outside this oracle's pre-existing rank-admission surface.
        redundant = [levels[0], 1]
        nested_categories = levels[0]
    policy_rank = sum(levels) - sum(redundant)
    df = m - k - policy_rank
    fit_df, correction_df = df - nested_categories, df - int(nested_categories > 0)
    _require(min(df, fit_df, correction_df) > 0,
             'nonpositive independently derived policy residual degrees of freedom')
    scores = influence * (w * residual)[:, None]
    if vce == 'robust':
        covariance = scores.T @ scores * (m / df)
    elif vce == 'iid':
        covariance = (influence.T @ influence) * (residual @ residual / df)
    else:
        cluster_codes = np.unique(clusters[active], return_inverse=True)[1]
        groups = int(cluster_codes.max()) + 1
        _require(groups >= 2, 'cluster inference requires at least two retained groups')
        sums = np.zeros((groups, scores.shape[1]))
        np.add.at(sums, cluster_codes, scores)
        covariance = (sums.T @ sums) * ((m-1)/correction_df * groups/(groups-1))
    _require(np.all(np.isfinite(reported)) and np.all(np.isfinite(covariance)),
             'nonfinite independent coefficients or covariance')
    full_moment = _moments(codes, residual[:, None], w, y[:, None])
    finite_numerator = np.abs(x.T @ (w*residual))
    finite_scale = np.maximum(np.sqrt(np.sum(w[:, None]*x*x, axis=0) * np.sum(w*y*y)),
                              np.finfo(float).tiny)
    _require(np.all(np.isfinite(finite_numerator)) and np.all(np.isfinite(finite_scale))
             and np.all(finite_scale > 0), 'nonfinite or nonpositive finite-design moment normalization')
    finite_moment = np.max(finite_numerator / finite_scale)
    _require(np.isfinite(finite_moment), 'nonfinite finite-design moment ratio')
    _require(max(full_moment, finite_moment) <= MOMENT_LIMIT, 'original full-design residual moments failed')
    backward_numerator = np.max(np.abs(right_rhs[keep] - reduced @ solution))
    backward_scale = max(
        np.max(np.abs(right_rhs[keep])) + np.linalg.norm(reduced, np.inf)*np.max(np.abs(solution)),
        np.finfo(float).tiny)
    _require(np.isfinite(backward_numerator) and np.isfinite(backward_scale) and backward_scale > 0,
             'nonfinite or nonpositive direct-solve backward normalization')
    backward = backward_numerator / backward_scale
    _require(np.isfinite(backward) and backward <= MOMENT_LIMIT, 'direct grounded solve backward error failed')
    xb = x @ beta + (reported[-1] if include_intercept else 0.)
    arrays = dict(sample=active, drop_reason=np.where(active, 0, 8).astype(np.uint8))
    for name, values in [('residual', residual), ('fitted', fitted), ('xb', xb), ('absorbed', fitted-xb)]:
        arrays[name] = np.full(n, np.nan)
        arrays[name][active] = values
    dof = {0:m, 1:m, 2:n, 3:m, 5:k, 6:k, 7:policy_rank, 8:k+policy_rank,
           9:df, 10:fit_df, 11:correction_df, 12:nested_categories, 13:sum(levels), 14:sum(redundant),
           15:n-m, 16:n-m, 18:int(include_intercept), 19:int(q>2)}
    fe_rows = [[1, j+1, k+j+1, levels[j], redundant[j], levels[j]-redundant[j], int(j<2), int(account_cluster_nesting and j==0)]
               for j in range(q)]
    reference = dict(schema=1, kind=KIND, status='passed', rows=n, retained_rows=m, regressors=k,
        beta=reported.tolist(), covariance=covariance.tolist(), include_intercept=include_intercept,
        keep_singletons=keep_singletons, true_fe_rank=actual_rank, pairwise_fe_rank=policy_rank,
        fe_rows=fe_rows, dof_rows={str(key): value for key,value in dof.items()},
        certificate=dict(eliminated_factor=eliminated, levels=levels, retained_dimension=nr,
            grounded_dimension=len(keep), components=int(components), known_nullity=len(grounds),
            schur_spectrum=spectrum, finite_spectrum=xspectrum,
            anchored_fe_eigenvalue_lower_bound=h_lower, projection_error_bounds=errors.tolist(),
            projection_moment=projection_moment, full_fe_moment=full_moment,
            finite_moment=float(finite_moment), backward_error=float(backward), bread_inverse_error=inverse_error,
            modeled_peak_bytes=modeled, schur_product_work=work,
            weight_ratio=float(w.max()/w.min()), normalized_weight_sum=float(w.sum())))
    if memory_budget_bytes != MAX_BYTES:
        reference['certificate']['memory_budget_bytes'] = memory_budget_bytes
    if vce != 'robust':
        # A PSD certificate, not a covariance repair or a rank selection.
        eigenvalues = np.linalg.eigvalsh((covariance + covariance.T)/2)
        margin = 256*np.finfo(float).eps*len(eigenvalues)*max(float(np.max(np.abs(eigenvalues))), np.finfo(float).tiny)
        _require(np.isfinite(margin) and eigenvalues[0] >= -margin, 'independent covariance is not PSD')
        reference.update(vce=vce, covariance_psd=dict(minimum_eigenvalue=float(eigenvalues[0]), tolerance=margin))
        if vce == 'cluster':
            reference['cluster_count'] = groups
            reference['dof_rows']['9'] = min(df, groups-1)
            if account_cluster_nesting:
                reference.update(account_cluster_nesting=True, unclustered_residual_dof=df)
    return reference, arrays


def write_reference(case, output, *, memory_budget_bytes=MAX_BYTES):
    import scipy
    data = read_case(case)
    reference, arrays = solve(data['y'], data['x'], data['factors'], data['weights'],
                             include_intercept=data['include_intercept'], keep_singletons=data['keep_singletons'],
                             vce=data['vce'], clusters=data['clusters'],
                             account_cluster_nesting=data['account_cluster_nesting'],
                             memory_budget_bytes=memory_budget_bytes)
    output = Path(output)
    npz = output.with_suffix('.npz')
    _require(not output.exists() and not npz.exists(), 'reference output already exists')
    np.savez_compressed(npz, **arrays)
    reference.update({key: data[key] for key in ['case_sha256','body_sha256','columns','outputs']})
    reference.update(arrays={'file': npz.name, 'sha256': digest(npz)},
                     producer_sha256=digest(Path(__file__)), parser_sha256=digest(PARSER_SOURCE),
                     numpy=np.__version__, scipy=scipy.__version__)
    output.write_text(json.dumps(reference, sort_keys=True, indent=2, allow_nan=False) + '\n')
    return reference


def load_reference(path: Path, case_sha256: str, body_sha256: str):
    path = Path(path).resolve(strict=True)
    _require(path.stat().st_size <= 1_000_000, 'oversized oracle reference')
    reference = json.loads(path.read_text())
    _require(all(isinstance(value, str) and re.fullmatch(r'[a-f0-9]{64}', value)
                 for value in (case_sha256, body_sha256)), 'oracle reference identity requires input and body digests')
    _require(reference.get('schema') == 1 and reference.get('kind') == KIND
             and reference.get('status') == 'passed' and reference.get('case_sha256') == case_sha256
             and reference.get('body_sha256') == body_sha256, 'oracle reference identity mismatch')
    # Source digests record which implementation produced the witness. Editing
    # tooling prose or framing does not invalidate its mathematical result.
    # The qualification manifest binds this descriptor, its arrays and input.
    _require(type(reference.get('rows')) is int and 0 < reference['rows'] <= MAX_ROWS, 'invalid oracle row bound')
    cert = reference['certificate']
    allowed_bytes = memory_budget(cert.get('memory_budget_bytes', MAX_BYTES))
    levels, q = cert['levels'], len(cert['levels'])
    _require(2 <= q <= MAX_FE and all(type(v) is int and v > 0 for v in levels)
             and cert['known_nullity'] == (q-1)*cert['components']
             and reference['true_fe_rank'] == sum(levels)-cert['known_nullity']
             and 0 < cert['grounded_dimension'] <= cert['retained_dimension'] <= MAX_RETAINED
             and cert['grounded_dimension'] == cert['retained_dimension']-cert['known_nullity']
             and cert['modeled_peak_bytes'] <= allowed_bytes and cert['schur_product_work'] <= MAX_SCHUR_WORK
             and 1 <= cert['weight_ratio'] <= MAX_WEIGHT_RATIO,
             'invalid independent rank/resource certificate')
    for key in ['projection_moment','full_fe_moment','finite_moment','backward_error','bread_inverse_error']:
        _require(0 <= cert[key] <= MOMENT_LIMIT, 'independent numerical certificate failed: '+key)
    for key,dimension in [('schur_spectrum',cert['grounded_dimension']),('finite_spectrum',reference['regressors'])]:
        spectrum = cert[key]
        minimum, maximum = spectrum['minimum_eigenvalue'], spectrum['maximum_eigenvalue']
        eigen_margin = 256*np.finfo(float).eps*dimension*maximum
        formation = spectrum['formation_error_bound']
        margin = eigen_margin + formation
        _require(np.isfinite(maximum) and np.isfinite(formation) and formation >= 0
                 and minimum > margin > 0 and spectrum['eigensolver_margin'] == eigen_margin
                 and spectrum['required_margin'] == margin,
                 'independent spectrum certificate failed')
    _require(np.isfinite(cert['anchored_fe_eigenvalue_lower_bound'])
             and cert['anchored_fe_eigenvalue_lower_bound'] > 0
             and len(cert['projection_error_bounds']) == reference['regressors']+1
             and all(np.isfinite(v) and v >= 0 for v in cert['projection_error_bounds']),
             'invalid original-design projection error certificate')
    npz = (path.parent / reference['arrays']['file']).resolve(strict=True)
    _require(digest(npz) == reference['arrays']['sha256'], 'oracle arrays changed')
    with zipfile.ZipFile(npz) as archive:
        _require(sum(info.file_size for info in archive.infolist()) <= reference['rows']*80 + 100_000,
                 'oracle archive exceeds row-array bound')
    reference['_arrays_path'] = npz
    nesting = reference.get('account_cluster_nesting', False)
    _require(type(nesting) is bool and (not nesting or reference.get('vce') == 'cluster'),
             'invalid reference nesting policy')
    if reference.get('vce', 'robust') != 'robust':
        _require(reference['vce'] in {'iid', 'cluster'}, 'invalid reference VCE')
        psd = reference['covariance_psd']
        covariance = np.asarray(reference['covariance'], dtype=float)
        width = reference['regressors']+int(reference['include_intercept'])
        _require(covariance.shape == (width, width) and np.all(np.isfinite(covariance)),
                 'invalid reference covariance shape/values')
        values = np.linalg.eigvalsh((covariance+covariance.T)/2)
        margin = 256*np.finfo(float).eps*width*max(float(np.max(np.abs(values))), np.finfo(float).tiny)
        _require(np.isfinite(psd['tolerance']) and psd['tolerance'] > 0
                 and np.isfinite(psd['minimum_eigenvalue'])
                 and psd['minimum_eigenvalue'] >= -psd['tolerance']
                 and values[0] >= -margin
                 and np.isclose(psd['tolerance'], margin, rtol=1e-12, atol=0)
                 and abs(psd['minimum_eigenvalue']-values[0]) <= margin,
                 'invalid covariance PSD certificate')
        if reference['vce'] == 'cluster':
            base_df = reference['dof_rows']['11']
            if reference.get('account_cluster_nesting', False):
                k, m = reference['regressors'], reference['retained_rows']
                base_df = m-k-(levels[1]-1)
                expected_fe = [[1,1,k+1,levels[0],levels[0],0,1,1],
                               [1,2,k+2,levels[1],1,levels[1]-1,1,0]]
                expected_dof = {'7':levels[1]-1, '8':k+levels[1]-1, '10':base_df-levels[0],
                                '11':base_df-1, '12':levels[0], '13':sum(levels),
                                '14':levels[0]+1, '19':0}
                _require(reference['account_cluster_nesting'] is True and q == 2
                         and reference['cluster_count'] == levels[0]
                         and reference['unclustered_residual_dof'] == base_df
                         and reference['pairwise_fe_rank'] == levels[1]-1
                         and reference['fe_rows'] == expected_fe
                         and all(reference['dof_rows'][key] == value for key,value in expected_dof.items())
                         and min(base_df,base_df-levels[0],base_df-1) > 0,
                         'invalid nested cluster DoF certificate')
            _require(type(reference['cluster_count']) is int and 2 <= reference['cluster_count'] <= reference['retained_rows']
                     and reference['dof_rows']['9'] == min(base_df, reference['cluster_count']-1),
                     'invalid cluster count/DoF certificate')
    return reference, {path:digest(path), npz:reference['arrays']['sha256']}


def check_reference(reference, raw: dict, columns: Path, meta: dict):
    _require(meta['rows'] == reference['rows'] and meta['columns'] == reference['columns']
             and meta['outputs'] == reference['outputs'], 'oracle output layout mismatch')
    tables = raw['matrices']
    def table(name):
        t = tables[name]
        return np.asarray(numbers(t)).reshape(t['rows'], t['columns'])
    def close(a, b, label, rtol, atol):
        _require(np.shape(a) == np.shape(b) and np.allclose(a,b,rtol=rtol,atol=atol), 'independent oracle '+label+' mismatch')
    close(table('RB').ravel(), reference['beta'], 'coefficients', 1e-7, 1e-10)
    close(table('RV'), reference['covariance'], reference.get('vce', 'HC1')+' covariance', 1e-6, 1e-10)
    if reference.get('vce') == 'cluster':
        close(table('RCLUSTER'), [[1,1,reference['cluster_count'],0]], 'cluster metadata', 0, 0)
        close(table('RSUBSET'), [[1,1,1,reference['cluster_count']]], 'cluster subset metadata', 0, 0)
    close(table('RFE'), reference['fe_rows'], 'FE metadata/ranks', 0, 0)
    dof = table('RDOF')
    _require(dof.shape == (20,2), 'unsupported native DoF layout')
    for key,value in reference['dof_rows'].items():
        close(dof[int(key)], [1,value], 'DoF row '+key, 0, 0)
    expected = [1]*reference['regressors'] + ([5] if reference['include_intercept'] else [])
    close(table('RSTATUS')[:,1], expected, 'coefficient status', 0, 0)
    with np.load(reference['_arrays_path'], allow_pickle=False) as oracle:
        _require(set(oracle.files) == {'sample','drop_reason','residual','fitted','xb','absorbed'}, 'invalid oracle row arrays')
        active = oracle['sample']
        _require(active.shape == (meta['rows'],) and active.dtype == bool, 'invalid independent sample')
        data = np.memmap(columns, dtype=np.float64, mode='r', shape=(meta['columns'],meta['rows']))
        try:
            for key in ['sample','drop_reason']:
                close(data[meta['outputs'][key]], oracle[key], key, 0, 0)
            for key,target in [('residual','residual'),('xbd','fitted'),('xb','xb'),('absorbed','absorbed')]:
                observed = data[meta['outputs'][key]]
                close(observed[active], oracle[target][active], key, 1e-7, 1e-9)
                close(observed[active], oracle[target][active], 'absolute '+key, 0, 2e-8)
                _require(np.all(~np.isfinite(observed[~active]) | (observed[~active] >= 8e307)),
                         'excluded row has published numeric '+key)
        finally:
            del data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--memory-budget-gib', type=int, choices=(1, 2), default=1,
                        help='prospectively declared reference workspace allowance; default remains 1 GiB')
    args = parser.parse_args()
    from routing_cases import scratch
    _require(os.environ.get('OPENBLAS_NUM_THREADS') == '1' and os.environ.get('VECLIB_MAXIMUM_THREADS') == '1',
             'set OPENBLAS_NUM_THREADS=1 and VECLIB_MAXIMUM_THREADS=1 for the bounded oracle')
    output = scratch(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    def expired(*_):
        raise TimeoutError('independent oracle exceeded its 120-second execution bound')
    signal.signal(signal.SIGALRM, expired)
    signal.alarm(120)
    reference = write_reference(args.case, output, memory_budget_bytes=args.memory_budget_gib << 30)
    signal.alarm(0)
    print(json.dumps({key:reference[key] for key in ['kind','case_sha256','rows','retained_rows','true_fe_rank','pairwise_fe_rank','certificate']}))


if __name__ == '__main__':
    main()
