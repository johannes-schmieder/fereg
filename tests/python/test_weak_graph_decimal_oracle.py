import importlib.util
import json
import unittest
from decimal import Decimal
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "weak_graph_decimal_oracle.py"
SPEC = importlib.util.spec_from_file_location("weak_graph_decimal_oracle", MODULE_PATH)
ORACLE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(ORACLE)
SEALED = ROOT / "benchmarks" / "expected" / "weak_graph_decimal_oracle_v1.json"


class WeakGraphDecimalOracleTests(unittest.TestCase):
    def test_sealed_oracle_reproduces_bit_for_bit(self):
        sealed = json.loads(SEALED.read_text(encoding="utf-8"))
        ORACLE.verify(sealed)
        ORACLE.verify_flat_csv(sealed, SEALED.with_suffix(".csv").read_text(encoding="utf-8"))

    def test_required_fixture_dimensions_are_covered(self):
        sealed = ORACLE.generate()
        tags = {tag for fixture in sealed["fixtures"] for tag in fixture["tags"]}
        self.assertTrue(
            {
                "chain",
                "barbell",
                "disconnected",
                "akm",
                "high_mobility",
                "low_mobility",
                "unequal_weights",
                "multiple_rhs",
                "absorbed_slope",
            }.issubset(tags)
        )

    def test_high_precision_observation_space_audits_are_tiny(self):
        for fixture in ORACLE.generate()["fixtures"]:
            for rhs in fixture["rhs"]:
                self.assertLess(
                    abs(Decimal(rhs["observation_projection_relative_two_norm"])),
                    Decimal("1e-70"),
                    fixture["id"],
                )


if __name__ == "__main__":
    unittest.main()
