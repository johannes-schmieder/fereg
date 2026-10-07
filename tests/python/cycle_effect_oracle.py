"""High-precision conditional FE oracle for a connected Schur cycle.

Eliminate each left partition in Decimal arithmetic. Its two right neighbors
contribute one weighted edge. Ground one vertex and solve the remaining path
with tridiagonal elimination. This consumes original data and independently
estimated finite coefficients, never native fixed effects or solver state.
"""
import argparse
from decimal import Decimal, localcontext
import hashlib
import json
from pathlib import Path

import numpy as np
import pandas as pd


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def solve_fixed_beta(left, right, response, columns, weights, beta, precision=80):
    left, right = np.asarray(left), np.asarray(right)
    y, x, w, beta = [np.asarray(value, dtype=float) for value in [response, columns, weights, beta]]
    n = len(y)
    require(n > 0 and left.shape == right.shape == w.shape == y.shape == (n,), 'unaligned observations')
    require(x.shape == (len(beta), n) and beta.ndim == 1, 'unaligned finite columns')
    require(all(np.isfinite(value).all() for value in [left, right, y, x, w, beta]) and (w > 0).all(), 'invalid values or weights')
    require(precision >= 40, 'use at least forty decimal digits')
    _, left = np.unique(left, return_inverse=True)
    _, right = np.unique(right, return_inverse=True)
    nl, nr = int(left.max()) + 1, int(right.max()) + 1
    require(nr >= 3, 'a simple cycle needs at least three right vertices')
    order = np.argsort(left, kind='stable')
    counts = np.bincount(left)
    with localcontext() as context:
        context.prec = precision
        zero = Decimal(0)
        coefficients = [Decimal.from_float(float(value)) for value in beta]
        edges, groups = {}, []
        right_rhs = [zero for _ in range(nr)]
        right_weights = [zero for _ in range(nr)]
        start = 0
        for count in counts:
            neighbors = {}
            for row in order[start:start + count]:
                vertex = int(right[row])
                weight = Decimal.from_float(float(w[row]))
                target = Decimal.from_float(float(y[row]))
                for column, coefficient in enumerate(coefficients):
                    target -= coefficient * Decimal.from_float(float(x[column, row]))
                degree, moment = neighbors.get(vertex, (zero, zero))
                neighbors[vertex] = (degree + weight, moment + weight * target)
                right_weights[vertex] += weight
            start += count
            require(1 <= len(neighbors) <= 2, 'a left group has more than two right neighbors')
            degree = sum((value[0] for value in neighbors.values()), zero)
            moment = sum((value[1] for value in neighbors.values()), zero)
            groups.append((neighbors, degree, moment))
            if len(neighbors) == 2:
                a, b = sorted(neighbors)
                ca, sa = neighbors[a]
                cb, sb = neighbors[b]
                conductance = ca * cb / degree
                edge = (a, b)
                edges[edge] = edges.get(edge, zero) + conductance
                forcing = (cb * sa - ca * sb) / degree
                right_rhs[a] += forcing
                right_rhs[b] -= forcing
        adjacency = [[] for _ in range(nr)]
        for a, b in edges:
            adjacency[a].append(b)
            adjacency[b].append(a)
        require(all(len(neighbors) == 2 for neighbors in adjacency), 'the Schur graph is not degree two')
        walk, previous, current = [0], 0, min(adjacency[0])
        while current != 0 and len(walk) < nr:
            require(current not in walk, 'cycle walk repeated a vertex')
            walk.append(current)
            neighbors = adjacency[current]
            following = neighbors[0] if neighbors[1] == previous else neighbors[1]
            previous, current = current, following
        require(current == 0 and len(walk) == nr, 'the Schur graph contains disconnected cycles')

        def edge_weight(a, b):
            return edges[tuple(sorted((a, b)))]

        upper, solved_rhs = [], []
        for position, vertex in enumerate(walk[1:], start=1):
            before = walk[position - 1]
            after = walk[position + 1] if position + 1 < nr else 0
            lower = -edge_weight(before, vertex)
            diagonal = -lower + edge_weight(vertex, after)
            rhs = right_rhs[vertex]
            if position > 1:
                diagonal -= lower * upper[-1]
                rhs -= lower * solved_rhs[-1]
            require(diagonal > 0, 'grounded cycle pivot is not positive')
            upper.append(-edge_weight(vertex, after) / diagonal if after != 0 else zero)
            solved_rhs.append(rhs / diagonal)
        right_effect = [zero for _ in range(nr)]
        for position in range(nr - 2, -1, -1):
            vertex = walk[position + 1]
            after = walk[position + 2] if position + 2 < nr else 0
            right_effect[vertex] = solved_rhs[position] - upper[position] * right_effect[after]
        left_effect = [(moment - sum((degree_at_vertex * right_effect[vertex]
            for vertex, (degree_at_vertex, _) in neighbors.items()), zero)) / degree
            for neighbors, degree, moment in groups]
        total_weight = sum(right_weights, zero)
        left_mean = sum((degree * value for (_, degree, _), value in zip(groups, left_effect)), zero) / total_weight
        right_mean = sum((weight * value for weight, value in zip(right_weights, right_effect)), zero) / total_weight
        intercept = float(left_mean + right_mean)
        left_values = np.array([float(value - left_mean) for value in left_effect])[left]
        right_values = np.array([float(value - right_mean) for value in right_effect])[right]
    finite = np.zeros(n)
    for values, coefficient in zip(x, beta):
        finite += values * coefficient
    xb = finite + intercept
    prediction = xb + left_values + right_values
    expected = np.array([left_values, right_values, y - prediction, xb, prediction])
    require(np.isfinite(expected).all(), 'nonfinite expected output')
    return expected, dict(precision=precision, n=n, left_levels=nl, right_levels=nr,
        edges=len(edges), intercept=intercept, cycle_order_sha256=hashlib.sha256(np.asarray(walk, dtype='<i8').tobytes()).hexdigest())


def audit(results, oracle_receipt, output):
    require(not output.exists(), 'never overwrite a prior adjudication')
    evidence = json.loads(oracle_receipt.read_text())
    require(evidence['complete'] and evidence['oracle_source_sha256'] == sha(Path(__file__).with_name('graph_oracle.py')), 'independent oracle source changed')
    solved, records = {}, []
    for path in results:
        result = json.loads(path.read_text())
        require(result['status'] == 'pass' and result['arm'] == 'fereg', 'require a successful native application')
        cell = result['cell']
        require(cell['weight_kind'] == 'fw' and cell['outputs'] == 'all' and cell['effects'] == ['g1', 'g2'], 'unexpected scientific request')
        data = Path(json.loads((path.parent / 'request.json').read_text())['data'])
        digest = sha(data)
        require(digest == result['input_sha256'], 'original input changed')
        if digest not in solved:
            info = json.loads(data.with_suffix('.json').read_text())
            require(info['sha256'] == digest, 'input manifest changed')
            matrix = np.memmap(data.with_suffix('.bin'), dtype='<f8', mode='r', shape=(len(info['columns']), cell['n']))
            columns = {name: matrix[index] for index, name in enumerate(info['columns'])}
            for name, values in columns.items():
                require(hashlib.sha256(values.tobytes()).hexdigest() == info['column_sha256'][name], 'input column changed')
            witnesses = [attempt['oracle'] for record in evidence['records'] for attempt in record['attempts']
                if attempt.get('input_sha256') == digest and attempt['status'] == 'pass'
                and attempt['oracle']['vce'] == cell['vce']]
            require(bool(witnesses) and all(witness == witnesses[0] for witness in witnesses), 'missing or conflicting finite-coefficient witness')
            witness = witnesses[0]
            require(witness['vce'] == cell['vce'] and witness['n'] == int(columns['w'].sum()), 'inference or frequency semantics changed')
            require(np.array_equal(columns['w'], columns['w'].astype(np.int64)), 'frequency weights must be integral')
            x = np.array([columns[f'x{index}'] for index in range(1, cell['k'] + 1)])
            expected, metadata = solve_fixed_beta(columns['g1'], columns['g2'], columns['y'], x, columns['w'], witness['beta'], 80)
            lower_precision, _ = solve_fixed_beta(columns['g1'], columns['g2'], columns['y'], x, columns['w'], witness['beta'], 50)
            require(np.array_equal(expected, lower_precision), 'fifty and eighty digits do not agree after float conversion')
            solved[digest] = (expected, metadata, witness['beta'])
        expected, metadata, beta = solved[digest]
        actual = np.fromfile(path.parent / 'outputs.bin', dtype='<f8').reshape(expected.shape)
        checks = []
        for name, values, target in zip(['fe1', 'fe2', 'residual', 'xb', 'prediction'], actual, expected):
            equal = np.isclose(values, target, rtol=1e-7, atol=1e-9)
            checks.append(dict(output=name, passing=bool(equal.all()), mismatches=int((~equal).sum()),
                maximum_absolute_error=float(np.max(np.abs(values - target)))))
        b = pd.read_csv(path.parent / 'b.csv').sort_values(['row', 'column']).value.to_numpy()
        record = dict(run=path.parent.name, input_sha256=digest, result_sha256=sha(path),
            outputs_sha256=sha(path.parent / 'outputs.bin'), independent_finite_beta=beta,
            expected_outputs_sha256=hashlib.sha256(expected.astype('<f8').tobytes()).hexdigest(),
            arithmetic=metadata, precision_50_80_float_equality=True, checks=checks,
            intercept_absolute_error=abs(float(b[-1]) - metadata['intercept']),
            status='pass' if all(check['passing'] for check in checks) else 'mismatch')
        records.append(record)
        print(record['run'], record['status'], [(check['output'], check['maximum_absolute_error']) for check in checks], flush=True)
    output.write_text(json.dumps(dict(schema='cycle-decimal-effect-adjudication-v1', source_sha256=sha(Path(__file__)),
        independent_inference_receipt_sha256=sha(oracle_receipt),
        tolerance=dict(relative=1e-7, absolute=1e-9), records=records), indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--oracle-receipt', type=Path, required=True)
    parser.add_argument('results', type=Path, nargs='+')
    args = parser.parse_args()
    audit(args.results, args.oracle_receipt, args.output)
