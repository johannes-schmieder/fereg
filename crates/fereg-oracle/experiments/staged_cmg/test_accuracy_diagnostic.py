import copy
import unittest

import accuracy_diagnostic as diagnostic


def fixture():
    b = diagnostic.bits
    return {
        "schema": 1, "n": 2, "iterations": 4, "repairs": 3, "trace": 1,
        "offsets": [0, 2], "firms": [0, 1], "labels": [0, 0], "sizes": [2],
        "weights_bits": [b(2.0), b(2.0)], "totals_bits": [b(4.0)],
        "target_bits": b(1e-8), "bnorm_bits": b(8.0**0.5),
        "initial_bits": [b(0.0), b(0.0)], "x_bits": [b(0.0), b(0.0)],
        "rhs_bits": [b(2.0), b(-2.0)], "recursive_bits": [b(0.0), b(0.0)],
        "truth_bits": [b(2.0), b(-2.0)], "direction_bits": [b(0.0), b(0.0)],
        "product_bits": [b(0.0), b(0.0)],
        "epochs": [{"update_offset": 0, "repairs": 0, "x_bits": [b(0.0), b(0.0)]}],
        "increment_bits": [[b(v), b(0.0)] for v in [1e16, 1.0, 1.0, -1e16]],
    }


class AccuracyDiagnosticTests(unittest.TestCase):
    def test_exact_witness_detects_accumulation_and_flattened_repair(self):
        result = diagnostic.audit(fixture())
        self.assertTrue(result["ordinary_update_replay_bit_exact"])
        candidates = result["candidates"]
        self.assertFalse(candidates["stored"]["passes_exact_captured_target"])
        for name in candidates.keys() - {"stored"}:
            self.assertTrue(candidates[name]["passes_exact_captured_target"], name)
        self.assertEqual(result["captured_truth_vs_exact_residual_error_norm"], "0")

    def test_final_epoch_keeps_its_original_corrected_anchor(self):
        capture = fixture()
        capture["epochs"].append({"update_offset": 3, "repairs": 1,
                                  "x_bits": [diagnostic.bits(1e16), diagnostic.bits(0.0)]})
        result = diagnostic.audit(capture)["candidates"]
        self.assertTrue(result["exact_sum_flattened"]["passes_exact_captured_target"])
        self.assertFalse(result["final_epoch_exact_sum_flattened"]["passes_exact_captured_target"])
        capture["epochs"][-1]["x_bits"][0] = diagnostic.bits(0.0)
        with self.assertRaisesRegex(ValueError, "epoch anchor"):
            diagnostic.audit(capture)

    def test_total_mismatch_is_rejected(self):
        capture = fixture()
        capture["totals_bits"] = [diagnostic.bits(3.0)]
        with self.assertRaisesRegex(ValueError, "stored worker total"):
            diagnostic.audit(capture)

    def test_unequal_weights_are_outside_this_witness(self):
        capture = fixture()
        capture["weights_bits"][0] = diagnostic.bits(1.0)
        with self.assertRaisesRegex(ValueError, "equal-weight"):
            diagnostic.audit(capture)

    def test_missing_update_or_changed_stored_iterate_is_rejected(self):
        for field in ["x_bits", "increment_bits"]:
            capture = copy.deepcopy(fixture())
            if field == "x_bits":
                capture[field][0] = diagnostic.bits(1.0)
            else:
                capture[field].pop()
            with self.assertRaises(ValueError):
                diagnostic.audit(capture)

    def test_nonfinite_input_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "nonfinite"):
            diagnostic.decode(diagnostic.bits(float("nan")))


if __name__ == "__main__":
    unittest.main()
