from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REBASE = ROOT / "legacy/fereg-core/src/ols/prepared_categorical_rebase.rs"
DATA = ROOT / "legacy/fereg-core/src/ols/prepared_categorical_data.rs"
RESULT_VIEW = ROOT / "crates/fereg-plugin/src/result_view.rs"


class FinalSampleRebaseStagingContractTests(unittest.TestCase):
    def test_continuous_rebase_preserves_candidate_storage_identity(self) -> None:
        text = REBASE.read_text(encoding="utf-8")
        self.assertIn("candidate_model: &PreparedOlsModel", text)
        self.assertIn("candidate_continuous_kind", text)
        self.assertIn("prepared_source_index: *prepared_source_index", text)
        self.assertNotIn("source.get() - 1", text)
        self.assertNotIn("fn source_index", text)

    def test_activation_passes_the_validated_availability_model(self) -> None:
        text = DATA.read_text(encoding="utf-8")
        self.assertIn("model: availability_model", text)
        self.assertIn("&availability_model,", text)
        self.assertIn(
            "final_categorical_sources(&availability_model, cluster_sources)", text
        )
        self.assertNotIn("remap_and_resolve_model", text)

    def test_transport_separates_candidate_capacity_from_final_graph(self) -> None:
        text = RESULT_VIEW.read_text(encoding="utf-8")
        self.assertIn("fn coefficient_capacity", text)
        self.assertIn("report_plan().original_coefficient_count()", text)

    def test_staged_rebase_remains_outside_public_dispatch(self) -> None:
        text = REBASE.read_text(encoding="utf-8")
        for token in ("estimate-ols", "fereg", "estimate-ppml", "feppml"):
            self.assertNotIn(token, text)
        self.assertNotIn("unsafe {", text)


if __name__ == "__main__":
    unittest.main()
