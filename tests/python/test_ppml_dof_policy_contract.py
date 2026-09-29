from __future__ import annotations

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
PPML = ROOT / "legacy/fereg-core/src/ppml"
RESULT = PPML / "result.rs"
ESTIMATOR = PPML / "prepared_estimator.rs"


class PpmlDofPolicyContractTests(unittest.TestCase):
    def test_production_ppml_never_imports_the_ols_dof_policy(self) -> None:
        for path in PPML.rglob("*.rs"):
            source = path.read_text(encoding="utf-8")
            self.assertNotIn(
                "PreparedOlsAbsorbedDofPolicy",
                source,
                f"{path.relative_to(PPML)} leaks OLS DoF policy into PPML",
            )

    def test_result_boundary_freezes_shared_pairwise_topology_policy(self) -> None:
        result = RESULT.read_text(encoding="utf-8")
        function = result.split(
            "pub(crate) fn prepare_ppml_fixed_effect_report(", 1
        )[1].split("\n}\n", 1)[0]
        signature = function.split(") -> Result<PreparedPpmlFixedEffectReport>", 1)[0]

        self.assertNotIn("policy", signature)
        self.assertEqual(
            function.count("CategoricalTopologyDofPolicy::Pairwise"), 1
        )
        for configurable in ("::None", "::FirstPair", "::Continuous"):
            self.assertNotIn(f"CategoricalTopologyDofPolicy{configurable}", function)

        estimator = ESTIMATOR.read_text(encoding="utf-8")
        call = estimator.split("prepare_ppml_fixed_effect_report(", 1)[1].split(
            ")?", 1
        )[0]
        self.assertNotIn("Policy", call)
        self.assertNotIn("Pairwise", call)


if __name__ == "__main__":
    unittest.main()
