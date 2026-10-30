"""Independent graph cuts, weighted dummy OLS and Poisson cell identities."""
import importlib.util
from pathlib import Path
import unittest

import numpy as np
from scipy.sparse import coo_matrix
from scipy.sparse.csgraph import connected_components

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    'bridge_optimum_witness',
    ROOT / 'crates/fereg-oracle/python/bridge_optimum_witness.py')
WITNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WITNESS)


class BridgeOptimumTests(unittest.TestCase):
    def test_bridge_flags_match_independent_edge_removal(self):
        for pairs in ([(0, 1), (1, 2), (2, 0), (2, 3), (3, 4)],
                      [(0, 1), (2, 3)],
                      [(0, 1), (1, 2), (2, 3), (3, 0)], []):
            left = np.array([a for a, b in pairs], dtype=np.int64)
            right = np.array([b for a, b in pairs], dtype=np.int64)
            actual, seen, finish, parent, edge = WITNESS.bridges(5, left, right)

            def components(mask):
                graph = coo_matrix((np.ones(int(mask.sum())),
                                    (left[mask], right[mask])), shape=(5, 5))
                return connected_components(graph, directed=False,
                                            return_labels=False)

            keep = np.ones(len(pairs), dtype=bool)
            initial = components(keep)
            for index in range(len(pairs)):
                keep[index] = False
                self.assertEqual(bool(actual[index]), components(keep) > initial)
                keep[index] = True
                if actual[index]:
                    child = np.flatnonzero(edge == index)[0]
                    inside = (seen >= seen[child]) & (seen < finish[child])
                    np.testing.assert_array_equal(
                        np.flatnonzero(inside[left] != inside[right]), [index])

    def test_duplicate_cells_and_loops_are_rejected(self):
        with self.assertRaises(ValueError):
            WITNESS.bridges(2, [0, 0], [1, 1])
        with self.assertRaises(ValueError):
            WITNESS.bridges(2, [0], [0])

    def test_single_row_bridge_with_weights_and_extra_columns(self):
        # Two cycles connected by a bridge. Its endpoints have degree three,
        # so singleton pruning does not explain the free observation direction.
        pairs = [(0, 3), (0, 4), (1, 3), (1, 4), (1, 5),
                 (2, 5), (2, 6), (7, 5), (7, 6)]
        left = np.array([a for a, b in pairs])
        right = np.array([b for a, b in pairs])
        flagged, *_ = WITNESS.bridges(8, left, right)
        np.testing.assert_array_equal(np.flatnonzero(flagged), [4])
        rng = np.random.default_rng(72)
        cells = np.repeat(np.arange(len(pairs)), np.where(flagged, 1, 4))
        design = np.eye(8)[left[cells]] + np.eye(8)[right[cells]]
        extra = np.column_stack((rng.normal(size=len(cells)),
                                 np.arange(len(cells)) % 3 == 0))
        design = np.column_stack((design, extra))
        outcome = rng.uniform(.5, 4, len(cells))
        weights = rng.uniform(.1, 3, len(cells))
        beta = np.linalg.lstsq(design * np.sqrt(weights)[:, None],
                               outcome * np.sqrt(weights), rcond=None)[0]
        prediction = design @ beta
        selected = cells == 4
        np.testing.assert_allclose(prediction[selected], outcome[selected],
                                   rtol=0, atol=2e-13)
        mean = np.exp(rng.normal(size=len(cells)))
        scale = np.dot(weights[selected], outcome[selected]) / np.dot(
            weights[selected], mean[selected])
        np.testing.assert_allclose(mean[selected] * scale, outcome[selected],
                                   rtol=0, atol=2e-15)

    def test_multirow_updates_only_certify_the_cell_score(self):
        outcome = np.array([1., 3.])
        prediction = np.array([2., 5.])
        weights = np.array([2., .5])
        ols = prediction + np.dot(weights, outcome - prediction) / weights.sum()
        poisson = prediction * np.dot(weights, outcome) / np.dot(weights, prediction)
        self.assertLess(abs(np.dot(weights, outcome - ols)), 1e-14)
        self.assertLess(abs(np.dot(weights, outcome - poisson)), 1e-14)
        self.assertGreater(np.max(abs(ols - outcome)), .1)
        self.assertGreater(np.max(abs(poisson - outcome)), .1)


if __name__ == '__main__':
    unittest.main()
