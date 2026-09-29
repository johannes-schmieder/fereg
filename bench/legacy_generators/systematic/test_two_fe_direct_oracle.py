"""Cross-check the reduced direct oracle against a full explicit-dummy SVD."""
import json
import numpy as np
from two_fe_direct_oracle import solve


def validate():
    rng = np.random.default_rng(202609289)
    records = []
    for disconnected in (False, True):
        n = 320
        g = np.arange(n) % 20
        h = rng.integers(0, 9, n)
        if disconnected:
            h += 9 * (g >= 10)
        X = rng.normal(size=(n, 4))
        y = X @ np.array([.4, -.3, .2, .1]) + np.sin(g) + np.cos(h) + rng.normal(size=n)
        for generation, w in enumerate((np.ones(n), np.exp(rng.uniform(-3, 3, n)),
                                        np.exp(rng.uniform(-6, 6, n)))):
            result = solve(y, X, g, h, w)
            D = np.column_stack([X, np.eye(20)[g], np.eye(int(h.max()) + 1)[h]])
            A = np.sqrt(w)[:, None] * D
            u, singular, vh = np.linalg.svd(A, full_matrices=False)
            rank = int((singular > max(A.shape) * np.finfo(float).eps * singular[0]).sum())
            coef = vh[:rank].T @ ((u[:, :rank].T @ (np.sqrt(w) * y)) / singular[:rank])
            residual = y - D @ coef
            bread = (vh[:rank, :4] / singular[:rank, None]).T @ (vh[:rank, :4] / singular[:rank, None])
            assert result['rank'] == rank
            np.testing.assert_allclose(result['coefficient'], coef[:4], rtol=1e-10, atol=1e-11)
            np.testing.assert_allclose(result['residual'], residual, rtol=1e-9, atol=1e-11)
            np.testing.assert_allclose(result['bread'], bread, rtol=1e-9, atol=1e-11)
            records.append(dict(disconnected=disconnected, weight_generation=generation, rank=rank,
                                coefficient_error=float(np.max(abs(result['coefficient'] - coef[:4]))),
                                residual_error=float(np.max(abs(result['residual'] - residual))),
                                bread_error=float(np.max(abs(result['bread'] - bread)))))
    try:
        solve(y, np.column_stack([X, X[:, 0]]), g, h)
    except ValueError as error:
        assert 'rank deficient' in str(error)
    else:
        raise AssertionError('rank-deficient controls must not yield an oracle result')
    no_controls = solve(y, X[:, :0], g, h)
    assert no_controls['coefficient'].size == 0
    return dict(passed=True, full_dummy_svd_cases=records, rank_deficiency_rejected=True,
                zero_controls_pass=True)


if __name__ == '__main__':
    print(json.dumps(validate(), indent=2, allow_nan=False))
