"""Strict capture and scope guards for the bounded arithmetic experiment."""
import copy
import unittest
from unittest.mock import patch

from scripts import ols_arithmetic_development as dev


def reference_payload():
    prefix = dev.capture.MACHINE_PREFIX
    lines = []
    for name in ("b", "V"):
        row = "r1" if name == "b" else "x1"
        lines.extend([
            f"{prefix}matrix_dimensions|science|{name}|1|1",
            f"{prefix}matrix|science|{name}|1|1|{row}|x1|+1.0000000000000X+000",
        ])
    for name in ("N", "df_a", "df_r", "rss", "r2"):
        lines.append(f"{prefix}scalar|science|{name}|+1.0000000000000X+000")
    return lines


class ArithmeticDevelopmentTests(unittest.TestCase):
    def test_width_inventory_keeps_original_manifest_separate(self):
        original = dev.MANIFEST
        width_manifest = dev.ROOT / "benchmarks/manifests/ols_width_development_v1.json"
        with patch.object(dev, "MANIFEST", width_manifest):
            scopes = dev.scopes()
            self.assertEqual(len(scopes), 6)
            self.assertEqual({s["k"] for s in scopes}, {64, 65})
            self.assertEqual({s["design"] for s in scopes}, {"one_fe"})
            self.assertEqual(sum(s["memory"] == "low" for s in scopes), 2)
        self.assertEqual(dev.MANIFEST, original)

    def test_bounded_inventory(self):
        scopes = dev.scopes()
        self.assertEqual(len(scopes), 38)
        self.assertEqual(len({s["id"] for s in scopes}), 38)
        self.assertEqual({s["threads"] for s in scopes}, {1, 7})
        self.assertEqual(sum(s["memory"] == "low" for s in scopes), 8)
        self.assertEqual(sum(bool(s["guard"]) for s in scopes), 6)

    def test_commands_preserve_estimator_policy(self):
        for scope in dev.scopes():
            self.assertIn("solver(auto)", dev.command(scope))
            self.assertNotIn("solver(", dev.command(scope, reference=True))
        no_fe = dev.scopes()[0]
        self.assertIn("noabsorb", dev.command(no_fe, reference=True))
        self.assertNotIn("absorb", dev.command(no_fe))

    def test_complete_reference(self):
        result = dev.parse_reference("\n".join(reference_payload()))
        self.assertEqual(result["matrices"]["b"]["values"], [1.0])

    def test_missing_duplicate_and_truncated_reference_fail(self):
        lines = reference_payload()
        for broken in (lines[1:], lines[:1] + lines[2:], lines[:-1], lines + lines[:1], lines + lines[1:2]):
            with self.subTest(broken=broken):
                with self.assertRaises(ValueError):
                    dev.parse_reference("\n".join(broken))

    def test_nonfinite_and_extra_cells_fail(self):
        lines = reference_payload()
        for value in (".", "+1.0000000000000X+999"):
            changed = copy.copy(lines)
            changed[1] = changed[1].rsplit("|", 1)[0] + "|" + value
            with self.assertRaises((ValueError, OverflowError)):
                dev.parse_reference("\n".join(changed))
        with self.assertRaises(ValueError):
            dev.parse_reference("\n".join(lines + [lines[1].replace("|1|1|r1", "|2|1|r1")]))

    def test_covariance_coordinate_stripes_must_match_coefficients(self):
        lines = reference_payload()
        lines[3] = lines[3].replace("|x1|x1|", "|wrong|x1|")
        with self.assertRaises(ValueError):
            dev.parse_reference("\n".join(lines))


if __name__ == "__main__":
    unittest.main()
