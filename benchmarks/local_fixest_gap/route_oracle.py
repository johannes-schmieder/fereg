"""Independent joint dummy-model adjudication for the two local disagreements."""
import argparse
import numpy as np
import pandas as pd
from scipy import sparse
from scipy.sparse.linalg import lsmr
from common import write_json
import run  # Reuse the frozen systematic oracle module path.
from oracle import dummy_matrix


def joint_oracle(data):
    frame = pd.read_stata(data, convert_categoricals=False)
    fixed, rank = dummy_matrix(frame, ['g1', 'g2'])
    controls = frame[[f'x{i}' for i in range(1, 51)]].to_numpy()
    outcome = frame.y.to_numpy()
    design = sparse.hstack([sparse.csr_matrix(controls), fixed], format='csr')
    norms = np.sqrt(np.asarray(design.power(2).sum(axis=0)).ravel())
    scaled = design.multiply(1 / norms).tocsr()
    solved = lsmr(scaled, outcome, atol=1e-14, btol=1e-14, maxiter=10000)
    if solved[1] not in [1, 2, 4, 5]:
        raise RuntimeError(f'Explicit joint model did not converge: {solved[1]}')
    coefficients = solved[0] / norms
    residual = outcome - design @ coefficients
    return {'schema': 'explicit-joint-ols-local-v1', 'observations': len(outcome),
            'controls': 50, 'fe_rank': rank, 'df_r': len(outcome) - rank - 50,
            'iterations': int(solved[2]), 'stop': int(solved[1]),
            'coefficients': coefficients[:50].tolist(), 'rss': float(residual @ residual)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('data')
    parser.add_argument('output')
    args = parser.parse_args()
    from pathlib import Path
    write_json(Path(args.output), joint_oracle(args.data))
