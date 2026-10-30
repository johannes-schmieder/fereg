import importlib.util, math
from pathlib import Path
import numpy as np

here = Path(__file__).resolve().parents[2] / "crates/fereg-oracle/python"


def load(file, name):
    spec = importlib.util.spec_from_file_location(name, here / file)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


m = load("panel_optimum_enclosure.py", "enclosure")
bridge = None
import unittest


class PanelEnclosureTests(unittest.TestCase):
    def test_dense_ols_and_constructed_poisson_optima(self):
        checks = 0
        for seed in (3, 17, 65):
            rng = np.random.default_rng(seed)
            nw = 40
            n = 5 * nw
            nf = 8
            firm = np.repeat((np.arange(nw) % nf)[:, None], 5, axis=1)
            moving = np.arange(nw) % 4 == 0
            firm[moving, 3:] = rng.integers(0, nf, (moving.sum(), 2))
            third = np.arange(n) % 32
            rng.shuffle(third)
            X = rng.normal(size=(n, 16))
            Z = np.column_stack((X, np.eye(32)[third, :31]))
            D = np.column_stack(
                (np.eye(nw)[np.repeat(np.arange(nw), 5)], np.eye(nf)[firm.ravel()])
            )
            A = np.column_stack((D, Z))
            theta = rng.normal(size=A.shape[1]) * 0.03
            eta = A @ theta
            null = rng.normal(size=n)
            null -= A @ np.linalg.lstsq(A, null, rcond=None)[0]
            for poisson in (False, True):
                expected = np.exp(eta) if poisson else eta
                y = expected + null * 0.01
                assert not poisson or np.all(y > 0)
                for perturbation in (1e-10, 1e-8, 1e-4):
                    delta = rng.normal(size=A.shape[1]) * perturbation
                    approx = (
                        A @ (theta + delta) + rng.normal(size=n) * perturbation * 0.3
                    )
                    pred = np.exp(approx) if poisson else approx
                    beta = (theta + delta)[D.shape[1] : D.shape[1] + 16]
                    data = np.zeros((28 if poisson else 27, n))
                    data[1] = y
                    data[2:18] = X.T
                    data[18] = np.repeat(np.arange(1, nw + 1), 5)
                    data[19] = firm.ravel() + 1
                    data[20] = third + 1
                    data[26 if poisson else 25] = approx
                    w = pred if poisson else np.ones(n)
                    top = m.topology(firm, np, bridge)
                    e, pair = m.pair_correction(y, pred, w, firm, top, np)
                    recovered, global_part = m.gram_and_score(
                        data, y, pred, w, e, firm, beta, np
                    )
                    distance, feasible = m.feasibility(
                        data, beta, recovered, firm, top, w, np
                    )
                    E = (
                        pair["norm"]
                        + global_part["norm"]
                        + pair["round_norm"]
                        * (
                            1
                            + math.sqrt(
                                global_part["all_design_energy_bound"]
                                / global_part["certified_minimum_eigenvalue"]
                            )
                        )
                    )
                    residual = y - pred
                    transformed = A * np.sqrt(w)[:, None]
                    projection = (
                        transformed
                        @ np.linalg.lstsq(
                            transformed, residual / np.sqrt(w), rcond=None
                        )[0]
                    )
                    assert np.linalg.norm(projection) <= E * (1 + 1e-8) + 1e-14, (
                        seed,
                        poisson,
                        "dual",
                        np.linalg.norm(projection),
                        E,
                    )
                    if poisson:
                        d = float(distance.max()) + 32 * np.finfo(float).eps
                        dd = (
                            np.asarray(distance, dtype=float) + 32 * np.finfo(float).eps
                        )
                        R = (
                            2
                            * math.exp(d)
                            * (E + np.linalg.norm(np.sqrt(w) * np.expm1(dd)))
                        )
                        assert R / math.sqrt(w.min()) < math.log(2)
                        R *= 0.5 * math.exp(R / math.sqrt(w.min()))
                        limit = pred * np.expm1(dd + R / np.sqrt(w))
                        coordinate = R / math.sqrt(
                            global_part["certified_minimum_eigenvalue"]
                        )
                    else:
                        Dnorm = float(np.linalg.norm(distance))
                        limit = distance + E + Dnorm
                        coordinate = (E + Dnorm) / math.sqrt(
                            global_part["certified_minimum_eigenvalue"]
                        )
                    assert np.all(abs(pred - expected) <= limit + 1e-13), (
                        seed,
                        poisson,
                        "forward",
                    )
                    truth = theta[D.shape[1] : D.shape[1] + 16]
                    assert np.max(abs(beta - truth)) <= coordinate + 1e-13, (
                        seed,
                        poisson,
                        "coefficient",
                    )
                    checks += 1
        self.assertEqual(checks, 18)

    def test_summary_refinement_bounds_every_vector_component(self):
        rng = np.random.default_rng(83)
        p = np.exp(rng.normal(size=1000))
        distance = rng.uniform(0, 1e-11, len(p))
        radius = 1e-8
        relative = radius / np.sqrt(p.min())
        old = p * np.expm1(distance + radius / np.sqrt(p))
        tolerance = 1e-9 + 1e-7 * p
        ratio = float(np.max(old / tolerance))
        scan = dict(optimum=dict(kind='Poisson local curvature', admitted=True,
                    weighted_radius=radius, maximum_radius_over_sqrt_mean=relative,
                    feasible_mean_distance=float(np.linalg.norm(np.sqrt(p) * np.expm1(distance)))),
                    maximum_tolerance_ratio=ratio, maximum_failed_tolerance_ratio=ratio,
                    finite_coefficient_absolute_error_bound=1e-9,
                    finite_coefficient_minimum_tolerance=1e-8, original_failed_rows=19)
        refined = m.sharpen_poisson_summary(scan)
        actual = p * np.expm1(distance + radius * refined['radius_factor'] / np.sqrt(p))
        self.assertLessEqual(float(np.max(actual / tolerance)),
                             refined['maximum_tolerance_ratio_bound'])
        self.assertLess(refined['maximum_tolerance_ratio_bound'], ratio)

    def test_rejects_unadmitted_poisson_radius(self):
        with self.assertRaises(ValueError):
            m.sharpen_poisson_summary(
                dict(optimum=dict(kind="Poisson local curvature", admitted=False))
            )


if __name__ == "__main__":
    unittest.main()
