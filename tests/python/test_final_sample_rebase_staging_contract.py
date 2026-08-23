from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REBASE = ROOT / "crates/stfixest-core/src/ols/prepared_categorical_rebase.rs"
ACTIVATE = ROOT / "ci/source-transforms/activate_final_sample_rebase.py"


class FinalSampleRebaseStagingContractTests(unittest.TestCase):
    def test_continuous_rebase_preserves_candidate_storage_identity(self) -> None:
        text = REBASE.read_text(encoding="utf-8")
        self.assertIn("candidate_model: &PreparedOlsModel", text)
        self.assertIn("candidate_continuous_kind", text)
        self.assertIn("prepared_source_index: *prepared_source_index", text)
        self.assertNotIn("source.get() - 1", text)
        self.assertNotIn("fn source_index", text)

    def test_activation_passes_the_validated_availability_model(self) -> None:
        text = ACTIVATE.read_text(encoding="utf-8")
        self.assertIn("model: availability_model", text)
        self.assertIn("&availability_model,", text)
        self.assertIn(
            "final_categorical_sources(&availability_model, cluster_sources)", text
        )
        # The transform must retain exactly one copy of the old source anchor
        # that it replaces. Rejecting that literal globally is a false positive.
        self.assertEqual(text.count("mut model,"), 1)

    def test_staged_rebase_remains_outside_public_dispatch(self) -> None:
        text = REBASE.read_text(encoding="utf-8")
        for token in ("estimate-ols", "stfeols", "estimate-ppml", "stfepois"):
            self.assertNotIn(token, text)
        self.assertNotIn("unsafe {", text)


if __name__ == "__main__":
    unittest.main()
