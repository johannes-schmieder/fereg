from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIT_VIEW = ROOT / "crates/stfixest-core/src/ols/prepared_fit_view.rs"
OLS_MOD = ROOT / "crates/stfixest-core/src/ols/mod.rs"


class CategoricalFitViewSourceContractTests(unittest.TestCase):
    def test_shared_fit_view_covers_legacy_and_categorical_prepared_data(self) -> None:
        source = FIT_VIEW.read_text(encoding="utf-8")
        self.assertRegex(
            source,
            r"pub\(crate\)\s+trait\s+PreparedOlsFitView\b",
        )
        self.assertRegex(
            source,
            r"impl\s+PreparedOlsFitView\s+for\s+PreparedOlsData\b",
        )
        self.assertRegex(
            source,
            r"impl\s+PreparedOlsFitView\s+for\s+PreparedCategoricalOlsData\b",
        )

    def test_fit_view_remains_a_private_numerical_boundary(self) -> None:
        source = FIT_VIEW.read_text(encoding="utf-8")
        module = OLS_MOD.read_text(encoding="utf-8")

        self.assertRegex(module, r"(?m)^mod\s+prepared_fit_view\s*;")
        self.assertNotRegex(module, r"(?m)^pub\s+mod\s+prepared_fit_view\s*;")
        self.assertIsNone(re.search(r"\bpub\s+trait\s+PreparedOlsFitView\b", source))
        self.assertNotIn("estimate-ols", source)
        self.assertNotIn("stfeols", source)
        self.assertNotIn("unsafe {", source)


if __name__ == "__main__":
    unittest.main()
