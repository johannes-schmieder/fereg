"""Verify exported full-result invariants for the numeric-pass comparison."""
import argparse
import numpy as np
from final_compare import matrix
from common import read_json, write_json


def audit(root):
    checks = {}
    for path in sorted((root / 'runs').glob('final-*candidate*-fereg')):
        baseline = root / 'runs' / path.name.replace('candidate', 'baseline')
        if not baseline.exists():
            continue
        left = read_json(path / 'result.json')['metadata']
        right = read_json(baseline / 'result.json')['metadata']
        for name in ['b.csv', 'V.csv']:
            assert (path / name).read_bytes() == (baseline / name).read_bytes(), (path, name)
        keys = ['rss', 'tss', 'r2', 'r2_within', 'rmse', 'F', 'll', 'df_r', 'df_a', 'rank']
        matched = {key: left[key] == right[key] for key in keys if key in left and key in right}
        assert all(matched.values()), (path, matched)
        for name in ['fereg_fit', 'fereg_certificate']:
            assert np.array_equal(matrix(path, name), matrix(baseline, name)), (path, name)
        # Columns 9/10 are timing. The remaining fields include route,
        # iterations, stopping errors, fallback, preconditioner and probe.
        columns = [0, 1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 13, 14, 15]
        assert np.array_equal(matrix(path, 'fereg_solver')[:, columns],
                              matrix(baseline, 'fereg_solver')[:, columns]), path
        checks[path.name] = {'full_b_V_files_identical': True, 'fit_table_exact': True,
                            'certificate_table_exact': True, 'solver_except_times_exact': True,
                            'reported_statistics_exact': matched}
    write_json(root / 'extra-science.json', checks)
    print('Exact full-result and solver invariants:', len(checks), 'pairs')


if __name__ == '__main__':
    from pathlib import Path
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    audit(parser.parse_args().root)
