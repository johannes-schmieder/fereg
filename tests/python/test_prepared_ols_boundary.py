from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class PreparedOlsBoundaryContractTests(unittest.TestCase):
    def test_action_is_developer_guarded_and_packaged_as_internal_helper(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(encoding="utf-8")
        package = (ROOT / "stata/stfixest.pkg").read_text(encoding="utf-8")
        input_source = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("confirm=developer-preparation-test", helper)
        self.assertIn('const CONFIRMATION: &str = "developer-preparation-test"', input_source)
        self.assertIn("f _stfixest_prepare_ols_test.ado", package)

    def test_spi_session_is_explicitly_thread_confined(self):
        ffi = (ROOT / "crates/stfixest-plugin/src/ffi.rs").read_text(encoding="utf-8")
        self.assertIn("struct SpiSession", ffi)
        self.assertIn("PhantomData<Rc<()>>", ffi)
        self.assertIn("stfixest_spi_is_main_thread", ffi)

    def test_public_estimators_remain_fail_closed(self):
        dispatch = (ROOT / "crates/stfixest-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        for filename in ("stfeols.ado", "stfepois.ado"):
            text = (ROOT / "stata" / filename).read_text(encoding="utf-8").lower()
            self.assertIn("exit 498", text)
            self.assertNotIn("plugin call", text)

    def test_licensed_fixture_locks_missing_rows_and_retained_count(self):
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("r(n_retained) == 24", fixture)
        self.assertIn("inlist(_n, 2, 5, 6, 7)", fixture)
        self.assertIn("assert _rc == 498", fixture)


if __name__ == "__main__":
    unittest.main()
