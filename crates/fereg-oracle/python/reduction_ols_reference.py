"""Bounded independent explicit-dummy witness for the three declared E OLS controls."""
from pathlib import Path
import argparse
import hashlib
import json
import math
import os
import struct
import sys
os.environ.update(OPENBLAS_NUM_THREADS='1', OMP_NUM_THREADS='1', MKL_NUM_THREADS='1')
import numpy as np
from scipy.linalg import qr, solve_triangular

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'bench'))
sys.path.insert(0, str(ROOT / 'scripts'))
from native_io import case_header
from replay_plugins import matrix

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def derive(case):
    with case.open('rb') as source:
        shape, arguments = case_header(source)
        n, k, _, matrices, macros = shape
        assert (n, k, macros) == (32768, 27, 0)
        args = dict(value.decode().split('=', 1) for value in arguments[1:])
        columns = []
        for _ in range(k):
            assert source.readline() == b'0\n'
            columns.append(np.array([struct.unpack('>d', bytes.fromhex(source.readline().decode().strip()))[0] for _ in range(n)]))
    data = np.array(columns)
    assert args['vce'] == 'robust' and args['include_intercept'] == '1'
    assert args['candidate_all'] == '1' and np.all(data[0] == 1)
    assert np.all(np.isfinite(data[:21])) and np.all(data[20] > 0)
    y, controls, first, second, weight = data[1], data[2:18].T, data[18], data[19], data[20]
    assert len(np.unique(first)) == len(np.unique(second)) == 128
    # Independent dense QR; one second-effect anchor identifies the connected design.
    design = np.column_stack([first == level for level in np.unique(first)] +
                             [second == level for level in np.unique(second)[1:]] +
                             [controls[:, c] for c in range(16)])
    root = np.sqrt(weight)
    q, r = qr(root[:, None] * design, mode='economic', check_finite=True)
    assert np.min(np.abs(np.diag(r))) > 1e-8
    beta = solve_triangular(r, q.T @ (root * y))
    fitted = design @ beta
    residual = y - fitted
    # Finite-control influence is selected from the full dummy QR inverse.
    inverse = solve_triangular(r, q.T)[-16:] * root[None, :]
    score = inverse * residual[None, :]
    covariance = (score @ score.T) * (n / (n - design.shape[1]))
    # The leading unpivoted QR columns span only the explicit FE dummies.
    # Project the weighted outcome independently of the finite controls.
    fe_q = q[:, :255]
    weighted_outcome = root * y
    within_outcome = weighted_outcome - fe_q @ (fe_q.T @ weighted_outcome)
    within_tss = math.fsum(float(v) ** 2 for v in within_outcome) * n / math.fsum(weight)
    null_log_likelihood = -0.5 * n * (1.0 + math.log(2.0 * math.pi * within_tss / n))
    return {'input_sha256': digest(case), 'beta': beta[-16:], 'covariance': covariance,
            'fitted': fitted, 'residual': residual, 'rank': design.shape[1],
            'within_tss': within_tss, 'gaussian_null_log_likelihood': null_log_likelihood,
            'condition': float(np.linalg.cond(r))}

def compare(expected, result):
    data = json.loads(result.read_text())
    assert data['return_code'] == 0 and matrix(data, 'RCERT')[-1] == 1
    beta = np.array(matrix(data, 'RB'))[:16]
    cov = np.array(matrix(data, 'RV')).reshape(17, 17)[:16, :16]
    cols = np.fromfile(str(result) + '.columns', dtype=np.float64).reshape(27, 32768)
    assert np.all(cols[21] == 1) and np.all(cols[23] == 0)
    fit = matrix(data, 'RFIT')
    assert fit[4] == fit[22] == 1
    pairs = [('beta', beta, expected['beta'], 1e-7, 1e-10),
             ('covariance', cov, expected['covariance'], 1e-6, 1e-10),
             ('fitted', cols[25], expected['fitted'], 1e-7, 1e-9),
             ('residual', cols[22], expected['residual'], 1e-7, 1e-9),
             ('within_tss', np.array(fit[5]), expected['within_tss'], 1e-12, 1e-14),
             ('gaussian_null_log_likelihood', np.array(fit[23]), expected['gaussian_null_log_likelihood'], 1e-12, 1e-14)]
    errors = {}
    for name, actual, target, rtol, atol in pairs:
        gap = np.abs(actual - target)
        normalized = gap / (atol + rtol * np.maximum(np.abs(actual), np.abs(target)))
        assert np.all(normalized <= 1), (name, float(np.max(normalized)))
        errors[name] = {'max_absolute': float(gap.max()), 'maximum_fraction_of_bound': float(normalized.max())}
    return errors

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cases', type=Path, required=True)
    parser.add_argument('--results', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = []
    for name in ['ols-chain', 'ols-barbell', 'ols-akm-weak']:
        case = args.cases / (name + '.case')
        expected = derive(case)
        result = args.results / (case.stem + '.json')
        errors = compare(expected, result)
        np.savez(args.output / (case.stem + '-independent.npz'), **expected)
        rows.append({'case': case.name, 'input_sha256': expected['input_sha256'],
                     'rank': expected['rank'], 'condition': expected['condition'], 'baseline_errors': errors})
    (args.output / 'dense-witness.json').write_text(json.dumps(rows, indent=2) + '\n')
    print(json.dumps(rows, indent=2))
