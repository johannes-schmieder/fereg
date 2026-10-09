import importlib.util
from pathlib import Path
import unittest
try:
    import numpy as np
except ImportError as error:  # scientific Python is optional for the gate
    raise unittest.SkipTest(f"requires NumPy/SciPy/pandas: {error}") from error


SOURCE = Path(__file__).resolve().parents[2] / 'tests/python/cycle_effect_oracle.py'
SPEC = importlib.util.spec_from_file_location('cycle_effect_oracle', SOURCE)
oracle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(oracle)


def fixture():
    rng = np.random.default_rng(20260921)
    left, right = [], []
    group = 0
    for vertex in range(7):
        for _ in range(3):
            left.extend([group] * 3)
            right.extend([vertex, (vertex + 1) % 7, vertex])
            group += 1
        left.extend([group] * 2)
        right.extend([vertex] * 2)
        group += 1
    left, right = np.array(left), np.array(right)
    beta = np.array([0.3, -0.7])
    x = rng.normal(size=(2, len(left)))
    y = 1.25 + x.T @ beta + rng.normal(size=group)[left] + np.sin(2 * np.pi * right / 7) + rng.normal(size=len(left)) * .01
    return left, right, y, x, rng.integers(1, 7, size=len(left)), beta


def dense_oracle(left, right, y, x, weights, beta, literal=False):
    _, left = np.unique(left, return_inverse=True)
    _, right = np.unique(right, return_inverse=True)
    nl, nr = left.max() + 1, right.max() + 1
    a, b = np.eye(nl)[left], np.eye(nr)[right]
    design = np.column_stack([a, b[:, 1:]])
    target = y - x.T @ beta
    if literal:
        matrix = np.repeat(design, weights.astype(int), axis=0)
        response = np.repeat(target, weights.astype(int))
    else:
        roots = np.sqrt(weights)
        matrix, response = design * roots[:, None], target * roots
    coefficients, _, rank, _ = np.linalg.lstsq(matrix, response, rcond=None)
    assert rank == design.shape[1]
    first, second = a @ coefficients[:nl], b @ np.r_[0, coefficients[nl:]]
    intercept = np.average(first + second, weights=weights)
    center = np.average(second, weights=weights)
    first += center - intercept
    second -= center
    xb = x.T @ beta + intercept
    prediction = xb + first + second
    return np.array([first, second, y - prediction, xb, prediction])


class CycleEffectOracleTests(unittest.TestCase):
    def test_matches_grounded_dense_and_literal_frequency_equations(self):
        left, right, y, x, weights, beta = fixture()
        rng = np.random.default_rng(113)
        for shuffled in [False, True]:
            rows = rng.permutation(len(y)) if shuffled else np.arange(len(y))
            l = (rng.permutation(left.max() + 1) * 13 + 5)[left][rows]
            r = (rng.permutation(right.max() + 1) * 17 + 9)[right][rows]
            for w in [np.ones(len(y)), weights, weights * 3]:
                data = (l, r, y[rows], x[:, rows], w[rows], beta)
                actual, metadata = oracle.solve_fixed_beta(*data, precision=80)
                smaller, _ = oracle.solve_fixed_beta(*data, precision=50)
                np.testing.assert_array_equal(actual, smaller)
                np.testing.assert_allclose(actual, dense_oracle(*data), rtol=1e-10, atol=1e-11)
                np.testing.assert_allclose(actual, dense_oracle(*data, literal=True), rtol=1e-10, atol=1e-11)
                self.assertEqual(metadata['edges'], 7)
                self.assertLess(abs(np.average(actual[0], weights=w[rows])), 1e-13)
                self.assertLess(abs(np.average(actual[1], weights=w[rows])), 1e-13)

    def test_rejects_disconnected_cycles_extra_neighbors_and_invalid_weights(self):
        left, right, y, x, weights, beta = fixture()
        with self.assertRaisesRegex(ValueError, 'disconnected'):
            oracle.solve_fixed_beta(np.r_[left, left + 100], np.r_[right, right + 100],
                np.r_[y, y], np.column_stack([x, x]), np.r_[weights, weights], beta)
        with self.assertRaisesRegex(ValueError, 'more than two'):
            oracle.solve_fixed_beta(np.r_[left, [100] * 3], np.r_[right, [0, 1, 2]],
                np.r_[y, [0.] * 3], np.column_stack([x, np.zeros((2, 3))]), np.r_[weights, [1] * 3], beta)
        weights[0] = 0
        with self.assertRaisesRegex(ValueError, 'invalid'):
            oracle.solve_fixed_beta(left, right, y, x, weights, beta)


if __name__ == '__main__':
    unittest.main()
