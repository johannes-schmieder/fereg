"""Audit the scheduling comparison, including the diagnostic norm rounding."""
import argparse
from pathlib import Path
import numpy as np
from common import read_json, write_json
from final_compare import matrix


def audit(root):
    checks = {}
    for path in sorted((root / 'runs').glob('final-*candidate*-fereg')):
        baseline = root / 'runs' / path.name.replace('candidate', 'baseline')
        if not baseline.exists():
            continue
        a = read_json(path / 'result.json')
        b = read_json(baseline / 'result.json')
        assert a['status'] == b['status'] == 'pass'
        assert a['sample_sha256'] == b['sample_sha256']
        for name in ['b.csv', 'V.csv']:
            assert (path / name).read_bytes() == (baseline / name).read_bytes(), (path, name)
        for name in ['fereg_fit', 'fereg_certificate', 'fereg_status', 'fereg_dof']:
            assert np.array_equal(matrix(path, name), matrix(baseline, name)), (path, name)
        keys = ['rss', 'tss', 'r2', 'r2_within', 'rmse', 'F', 'll', 'df_r', 'df_a', 'rank',
                'fereg_memory_fast_bytes']
        exact = {key: a['metadata'][key] == b['metadata'][key]
                 for key in keys if key in a['metadata'] and key in b['metadata']}
        assert all(exact.values()), (path, exact)
        allocation_delta = (a['metadata']['fereg_native_allocation_bytes']
                            - b['metadata']['fereg_native_allocation_bytes'])
        if path.name.startswith('final-candidate-'):
            assert allocation_delta == 0, (path, allocation_delta)
        x, y = matrix(path, 'fereg_solver'), matrix(baseline, 'fereg_solver')
        # Only the diagnostic two-norm / normal-equation residual and timings
        # may differ. Acceptance maxima, iterations, routing and fallback agree.
        columns = [0, 1, 2, 3, 4, 5, 10, 11, 12, 13, 14, 15]
        assert np.array_equal(x[:, columns], y[:, columns]), (path, x-y)
        assert np.allclose(x[:, 6:8], y[:, 6:8], rtol=1e-10, atol=0.0), (path, x-y)
        changes = np.abs(x[:, 6:8] - y[:, 6:8])
        relative = changes / np.maximum(np.abs(y[:, 6:8]), np.finfo(float).tiny)
        checks[path.name] = {
            'full_b_V_files_identical': True, 'sample_status_DoF_exact': True,
            'fit_and_certificate_tables_exact': True, 'solver_acceptance_and_route_exact': True,
            'diagnostic_norm_max_relative_change': float(relative.max()),
            'reported_statistics_and_memory_budget_exact': exact,
            'native_peak_allocation_delta_bytes': allocation_delta,
        }
    assert checks, 'No candidate/baseline pairs found'
    write_json(root / 'extra-science.json', checks)
    print('Full-result scheduling audit:', len(checks), 'pairs')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    audit(parser.parse_args().root)
