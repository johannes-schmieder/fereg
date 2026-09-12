"""Check exact scientific invariants after input/preparation-only changes."""
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
        a, b = read_json(path / 'result.json'), read_json(baseline / 'result.json')
        assert a['status'] == b['status'] == 'pass'
        assert a['sample_sha256'] == b['sample_sha256']
        for name in ['b.csv', 'V.csv']:
            assert (path / name).read_bytes() == (baseline / name).read_bytes(), (path, name)
        for name in ['fereg_fit', 'fereg_certificate', 'fereg_status', 'fereg_dof']:
            assert np.array_equal(matrix(path, name), matrix(baseline, name)), (path, name)
        columns = [0, 1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 13, 14, 15]
        assert np.array_equal(matrix(path, 'fereg_solver')[:, columns],
                              matrix(baseline, 'fereg_solver')[:, columns]), path
        keys = ['rss', 'tss', 'r2', 'r2_within', 'rmse', 'F', 'll', 'df_r', 'df_a', 'rank',
                'fereg_memory_fast_bytes', 'fereg_memory_low_bytes']
        exact = {k: a['metadata'][k] == b['metadata'][k] for k in keys
                 if k in a['metadata'] and k in b['metadata']}
        assert all(exact.values()), (path, exact)
        checks[path.name] = {
            'full_b_V_files_identical': True, 'sample_status_DoF_exact': True,
            'fit_and_certificate_tables_exact': True, 'solver_except_times_exact': True,
            'reported_statistics_and_memory_budget_exact': exact,
            'native_peak_allocation_delta_bytes': a['metadata']['fereg_native_allocation_bytes']
                - b['metadata']['fereg_native_allocation_bytes'],
        }
    assert checks
    write_json(root / 'extra-science.json', checks)
    print('Exact scientific comparison:', len(checks), 'pairs')
    print('Native peak allocation deltas:', sorted(set(
        x['native_peak_allocation_delta_bytes'] for x in checks.values())))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    audit(parser.parse_args().root)
