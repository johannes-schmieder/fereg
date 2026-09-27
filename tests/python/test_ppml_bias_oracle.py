import sys
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
try:
    import numpy as np
    from ppml_bias_oracle import correction, fit_poisson, randomized_correction, philox_signs
except ImportError:
    np = None


@unittest.skipIf(np is None, "independent bias oracle requires scripts/requirements-ppml-bias.txt")
class PpmlBiasOracleTests(unittest.TestCase):
    def test_completed_duration_scalar(self):
        x = np.ones((4, 1))
        y = w = np.ones(4)
        beta, mu, _ = fit_poisson(x, y, w, np.array([4., 6., 10., 20.]))
        d, c = correction(x, y, w, mu, np.arange(4))
        self.assertAlmostEqual(float(np.exp(beta[0])), .1, places=14)
        self.assertAlmostEqual(float(d[0]), -.095, places=14)
        self.assertAlmostEqual(float(c[0]), .0475, places=14)

    def test_probe_identities_and_batch_partition(self):
        # Zero counter/key matches the public Random123 known-answer vector.
        self.assertEqual(int(philox_signs([0.], 0, 1, 0, 0)[0, 0]), 1)
        x = np.column_stack([np.ones(6), np.arange(6) / 5])
        y = np.array([1., 4., 2., 3., 8., 5.])
        w = np.array([2, 1, 3, 2, 1, 2])
        unit = np.array([0., 0., 1., 1., 2., 2.])
        _, mu, _ = fit_poisson(x, y, w, np.ones(6))
        for probes in (8, 31, 256, 1024):
            expected = randomized_correction(x,y,w,mu,unit,probes,827,1,batch=1)
            for batch in (3, 8, 32):
                actual = randomized_correction(x,y,w,mu,unit,probes,827,1,batch=batch)
                for a, e in zip(actual, expected):
                    np.testing.assert_allclose(a, e, rtol=1e-11, atol=1e-13)

    def test_unit_preserving_frequency_replication(self):
        x = np.column_stack([np.ones(6), np.arange(6) / 5])
        y = np.array([1., 4., 2., 3., 8., 5.])
        w = np.array([2, 1, 3, 2, 1, 2])
        unit = np.array([0, 0, 1, 1, 2, 2])
        _, mu, _ = fit_poisson(x, y, w, np.ones(6))
        expected = correction(x, y, w, mu, unit)
        actual = correction(np.repeat(x, w, axis=0), np.repeat(y, w),
                            np.ones(w.sum()), np.repeat(mu, w), np.repeat(unit, w))
        np.testing.assert_allclose(actual, expected, rtol=1e-12, atol=1e-13)
        scaled = correction(x, y, 13 * w, mu, unit)
        np.testing.assert_allclose(scaled, expected, rtol=1e-12, atol=1e-13)


if __name__ == "__main__":
    unittest.main()
