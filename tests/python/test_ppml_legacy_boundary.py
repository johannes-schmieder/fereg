from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "scripts/ppml_legacy_boundary.py"
SPEC = importlib.util.spec_from_file_location("ppml_legacy_boundary_test", PATH)
assert SPEC is not None and SPEC.loader is not None
BOUNDARY = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = BOUNDARY
SPEC.loader.exec_module(BOUNDARY)


class PpmlLegacyBoundaryTests(unittest.TestCase):
    def setUp(self) -> None:
        paths = {
            BOUNDARY.PPML_MOD,
            BOUNDARY.PPML_IRLS,
            BOUNDARY.PPML_PREPARED_SEPARATION,
            BOUNDARY.PPML_VCE,
            BOUNDARY.SEPARATION_MOD,
            f"{BOUNDARY.SEPARATION_RUNNER}/mod.rs",
            f"{BOUNDARY.SEPARATION_RUNNER}/detect.rs",
            f"{BOUNDARY.SEPARATION_RUNNER}/accounting.rs",
            "crates/fereg-core/src/lib.rs",
            "crates/fereg-core/src/ppml/prepared_irls.rs",
            "crates/fereg-plugin/src/ppml_estimate.rs",
        }
        self.documents = {
            relative: (ROOT / relative).read_text(encoding="utf-8")
            for relative in sorted(paths)
        }
        self.assertEqual(BOUNDARY.audit_documents(self.documents), [])

    def messages(self, documents: dict[str, str]) -> str:
        return "\n".join(str(item) for item in BOUNDARY.audit_documents(documents))

    def test_ungated_legacy_irls_module_is_rejected(self) -> None:
        documents = dict(self.documents)
        documents[BOUNDARY.PPML_MOD] = documents[BOUNDARY.PPML_MOD].replace(
            "#[cfg(test)]\nmod irls;", "mod irls;", 1
        )
        messages = self.messages(documents)
        self.assertIn("legacy module `irls` is not cfg(test)-only", messages)
        self.assertIn("legacy PPML token `irls` is reachable", messages)

    def test_every_stale_separation_module_is_cfg_test_only(self) -> None:
        source = self.documents[BOUNDARY.SEPARATION_MOD]
        for _path, module in BOUNDARY.MODULE_GATES[1:]:
            with self.subTest(module=module):
                documents = dict(self.documents)
                documents[BOUNDARY.SEPARATION_MOD] = source.replace(
                    f"#[cfg(test)]\nmod {module};", f"mod {module};", 1
                )
                self.assertIn(
                    f"legacy module `{module}` is not cfg(test)-only",
                    self.messages(documents),
                )

    def test_ppml_start_is_test_only_crate_private_and_not_reexported(self) -> None:
        documents = dict(self.documents)
        documents[BOUNDARY.PPML_MOD] = documents[BOUNDARY.PPML_MOD].replace(
            "#[cfg(test)]\n#[derive(Debug, Clone, PartialEq)]\npub(crate) struct PpmlStart",
            "#[derive(Debug, Clone, PartialEq)]\npub struct PpmlStart",
            1,
        )
        messages = self.messages(documents)
        self.assertIn("legacy struct `PpmlStart` is not cfg(test)-only", messages)
        self.assertIn("legacy struct `PpmlStart` is not crate-private", messages)
        self.assertIn("legacy PPML token `PpmlStart` is reachable", messages)

        documents = dict(self.documents)
        documents["crates/fereg-core/src/lib.rs"] += "\npub use ppml::PpmlStart;\n"
        self.assertIn(
            "legacy PPML token `PpmlStart` is reachable",
            self.messages(documents),
        )

    def test_ungated_renamed_fitter_cannot_escape_exact_inventory(self) -> None:
        documents = dict(self.documents)
        documents[BOUNDARY.PPML_MOD] = documents[BOUNDARY.PPML_MOD].replace(
            "#[cfg(test)]\npub(crate) fn fit_ppml(",
            "pub(crate) fn fit_poisson_compat(",
            1,
        )
        messages = self.messages(documents)
        self.assertIn("legacy fn `fit_ppml` inventory is not exact", messages)
        self.assertIn("legacy PPML token `fit_ppml_with_start` is reachable", messages)

    def test_production_wrapper_alias_is_rejected(self) -> None:
        documents = dict(self.documents)
        path = "crates/fereg-core/src/lib.rs"
        documents[path] += "\nuse crate::ppml::fit_ppml as production_fit;\n"
        messages = self.messages(documents)
        self.assertIn("legacy PPML token `fit_ppml` is reachable", messages)

    def test_internal_ppml_numerical_policy_cannot_be_reopened(self) -> None:
        for field in BOUNDARY.FROZEN_PPML_CONFIG_FIELDS:
            with self.subTest(field=field):
                documents = dict(self.documents)
                documents[BOUNDARY.PPML_MOD] = documents[BOUNDARY.PPML_MOD].replace(
                    f"pub(crate) {field}:", f"pub {field}:", 1
                )
                self.assertIn(
                    f"frozen PPML config field `{field}` is externally configurable",
                    self.messages(documents),
                )

        documents = dict(self.documents)
        documents[BOUNDARY.PPML_MOD] += "\npub enum PpmlInitialization { OffsetOnly, Intercept }\n"
        self.assertIn(
            "PPML initialization policy is externally configurable",
            self.messages(documents),
        )

        for name in BOUNDARY.FORBIDDEN_CRATE_ROOT_PPML_EXPORTS:
            with self.subTest(export=name):
                documents = dict(self.documents)
                documents["crates/fereg-core/src/lib.rs"] += f"\npub use ppml::{name};\n"
                self.assertIn(
                    f"internal PPML numerical symbol `{name}` is re-exported",
                    self.messages(documents),
                )

    def test_alternate_runner_module_name_and_path_are_rejected(self) -> None:
        documents = dict(self.documents)
        documents[BOUNDARY.SEPARATION_MOD] = documents[BOUNDARY.SEPARATION_MOD].replace(
            "#[cfg(test)]\nmod runner;",
            '#[path = "runner/mod.rs"]\nmod production_detector;',
            1,
        )
        messages = self.messages(documents)
        self.assertIn("legacy module `runner` declaration inventory is not exact", messages)
        self.assertIn("included through alternate path `runner/mod.rs`", messages)

    def test_parent_segments_cannot_hide_an_alternate_legacy_path(self) -> None:
        documents = dict(self.documents)
        documents[BOUNDARY.SEPARATION_MOD] += (
            '\n#[path = "../ppml/irls.rs"]\nmod renamed_dense_solver;\n'
        )
        self.assertIn(
            "included through alternate path `../ppml/irls.rs`",
            self.messages(documents),
        )

    def test_macro_built_textual_include_cannot_hide_legacy_module(self) -> None:
        documents = dict(self.documents)
        path = "crates/fereg-core/src/lib.rs"
        documents[path] += '\ninclude!(concat!("ppml/", "irls.rs"));\n'
        messages = self.messages(documents)
        self.assertIn("textual Rust include can bypass cfg(test) module gates", messages)

    def test_text_and_test_only_aliases_do_not_trigger_false_positives(self) -> None:
        documents = dict(self.documents)
        path = "crates/fereg-core/src/lib.rs"
        documents[path] += r'''
const LEGACY_DESCRIPTION: &str = "fit_ppml and ppml_separation::runner";
// fit_ppml_with_start is allowed in comments.
#[cfg(test)]
mod legacy_alias_test {
    use crate::ppml::fit_ppml as renamed_test_oracle;
}
'''
        self.assertEqual(BOUNDARY.audit_documents(documents), [])


if __name__ == "__main__":
    unittest.main()
