from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from scripts import fereg_allocation_source_contract as CONTRACT


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "benchmarks/manifests/fereg_allocation_source_contract_v4.json"


class FeregAllocationSourceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.contract = json.loads(MANIFEST.read_text(encoding="utf-8"))

    def _source_root(self, temporary: Path) -> Path:
        candidate = temporary / "candidate"
        for record in self.contract["cargo_wiring"]:
            relative = Path(record["path"])
            destination = candidate / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)
        for record in self.contract["native_route_wiring"]:
            relative = Path(record["path"])
            destination = candidate / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)
        for record in self.contract["source_files"]:
            relative = Path(record["path"])
            destination = candidate / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)
        ppml_caller = Path(CONTRACT.RETAINED_ROUTE_PPML_CALLER)
        ppml_destination = candidate / ppml_caller
        ppml_destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / ppml_caller, ppml_destination)
        return candidate

    @staticmethod
    def _replace(path: Path, old: str, new: str) -> None:
        source = path.read_text(encoding="utf-8")
        if source.count(old) != 1:
            raise AssertionError(f"mutation anchor is not unique in {path}: {old!r}")
        path.write_text(source.replace(old, new), encoding="utf-8")

    def test_current_source_matches_closed_inventory_and_exact_boundaries(self) -> None:
        self.assertEqual(CONTRACT.audit_repository(ROOT, MANIFEST), [])

    def test_pinned_preexisting_hashes_are_derived_from_exact_f0(self) -> None:
        for (relative, function), expected in CONTRACT.F0_PREEXISTING_BOUNDARY_HASHES.items():
            source = subprocess.run(
                ["git", "show", f"{CONTRACT.F0_SOURCE_SHA}:{relative}"],
                cwd=ROOT,
                check=True,
                stdout=subprocess.PIPE,
            ).stdout.decode("utf-8")
            body = CONTRACT._function_body(CONTRACT._production_tokens(source), function)
            observed = hashlib.sha256(
                CONTRACT._canonical_tokens(body).encode("utf-8")
            ).hexdigest()
            self.assertEqual(observed, expected, f"{relative}:{function}")
            literals = CONTRACT._production_literal_payloads(source, body)
            expected_count, expected_literal_digest = (
                CONTRACT.F0_PREEXISTING_BOUNDARY_LITERAL_HASHES[
                    (relative, function)
                ]
            )
            self.assertEqual(len(literals), expected_count, f"{relative}:{function}")
            self.assertEqual(
                hashlib.sha256(
                    CONTRACT._canonical_json(literals).encode("utf-8")
                ).hexdigest(),
                expected_literal_digest,
                f"{relative}:{function}",
            )
        for (relative, constant), expected in (
            CONTRACT.F0_ROUTE_LITERAL_BINDINGS.items()
        ):
            source = subprocess.run(
                ["git", "show", f"{CONTRACT.F0_SOURCE_SHA}:{relative}"],
                cwd=ROOT,
                check=True,
                stdout=subprocess.PIPE,
            ).stdout.decode("utf-8")
            self.assertEqual(
                CONTRACT._production_string_constant(source, constant),
                expected,
                f"{relative}:{constant}",
            )
        ffi_source = subprocess.run(
            [
                "git",
                "show",
                f"{CONTRACT.F0_SOURCE_SHA}:crates/fereg-plugin/src/ffi.rs",
            ],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout.decode("utf-8")
        ffi_branch = CONTRACT._ffi_ols_dispatch_branch_tokens(ffi_source)
        self.assertEqual(len(ffi_branch), CONTRACT.F0_FFI_OLS_BRANCH_TOKEN_COUNT)
        self.assertEqual(
            hashlib.sha256(
                CONTRACT._canonical_tokens(ffi_branch).encode("utf-8")
            ).hexdigest(),
            CONTRACT.F0_FFI_OLS_BRANCH_SHA256,
        )

    def test_comments_and_whitespace_do_not_change_the_structural_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            estimate = candidate / "crates/fereg-plugin/src/estimate.rs"
            self._replace(
                estimate,
                "pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {",
                "// formatting-only mutation\n\n"
                "pub fn estimate_ols_test( args: &[String], spi: &mut dyn SpiApi ) -> Result<()> {",
            )
            self.assertEqual(CONTRACT.audit_repository(candidate, MANIFEST), [])

    def test_direct_observation_sized_allocation_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            estimate = candidate / "crates/fereg-plugin/src/estimate.rs"
            self._replace(
                estimate,
                "    let started = Instant::now();",
                "    let rows = args.len();\n"
                "    let _new_observation_buffer = Vec::<f64>::with_capacity(rows);\n"
                "    let started = Instant::now();",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("allocation syntax inventory changed" in item for item in violations),
                violations,
            )
            self.assertTrue(
                any("exact fereg adapter/one-shot boundary changed" in item for item in violations),
                violations,
            )

    def test_public_fereg_error_path_allocation_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            error = candidate / "crates/fereg-core/src/error.rs"
            self._replace(
                error,
                "    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {\n"
                "        Self {",
                "    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {\n"
                "        let _hidden = vec![0_u8; 524_288];\n"
                "        Self {",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "allocation syntax inventory changed: "
                "crates/fereg-core/src/error.rs",
                violations,
            )
            self.assertIn(
                "guarded production token structure changed: "
                "crates/fereg-core/src/error.rs",
                violations,
            )

    def test_indirect_observation_by_column_allocation_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            estimate = candidate / "crates/fereg-plugin/src/estimate.rs"
            source = estimate.read_text(encoding="utf-8")
            helper = (
                "\nfn new_fereg_design_copy(rows: usize, columns: usize) -> Vec<f64> {\n"
                "    vec![0.0; rows * columns]\n"
                "}\n\n"
            )
            source = source.replace(
                "pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {",
                helper
                + "pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {\n"
                + "    let _new_design = new_fereg_design_copy(args.len(), args.len());",
                1,
            )
            estimate.write_text(source, encoding="utf-8")
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("allocation syntax inventory changed" in item for item in violations),
                violations,
            )
            self.assertTrue(
                any("exact fereg adapter/one-shot boundary changed" in item for item in violations),
                violations,
            )

    def test_ppml_import_on_public_ols_adapter_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            estimate = candidate / "crates/fereg-plugin/src/estimate.rs"
            estimate.write_text(
                "use fereg_core::fit_prepared_ppml;\n"
                + estimate.read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "public OLS adapter exposes forbidden PPML/session token fit_prepared_ppml",
                violations,
            )

    def test_ffi_import_alias_cannot_retarget_ols_to_ppml(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            ffi = candidate / "crates/fereg-plugin/src/ffi.rs"
            self._replace(
                ffi,
                "use crate::estimate::{estimate_ols_test, ACTION_ESTIMATE_OLS};",
                "use crate::estimate::ACTION_ESTIMATE_OLS;\n"
                "use crate::ppml_estimate::estimate_ppml_test as estimate_ols_test;",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("guarded production token structure changed" in item for item in violations),
                violations,
            )

    def test_plugin_module_path_cannot_replace_watched_estimate_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            lib = candidate / "crates/fereg-plugin/src/lib.rs"
            self._replace(
                lib,
                "mod estimate;",
                '#[path = "estimate_alt.rs"]\nmod estimate;',
            )
            alternate = candidate / "crates/fereg-plugin/src/estimate_alt.rs"
            alternate.write_text(
                "pub const ACTION_ESTIMATE_OLS: &str = \"estimate-ols\";\n"
                "pub use crate::ppml_estimate::estimate_ppml_test as estimate_ols_test;\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("guarded production token structure changed" in item for item in violations),
                violations,
            )
            self.assertTrue(
                any("watched fereg allocation source set changed" in item for item in violations),
                violations,
            )

    def test_plugin_cargo_lib_path_cannot_replace_the_compiled_route(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            cargo = candidate / "crates/fereg-plugin/Cargo.toml"
            self._replace(
                cargo,
                'path = "src/lib.rs"',
                'path = "src/ppml_estimate.rs"',
            )
            replacement = candidate / "crates/fereg-plugin/src/ppml_estimate.rs"
            replacement.write_text(
                "#[no_mangle]\n"
                "pub extern \"C\" fn fereg_dispatch() -> i32 { 0 }\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "Cargo target/dependency wiring changed: "
                "crates/fereg-plugin/Cargo.toml",
                violations,
            )

    def test_c_bridge_allocation_on_public_fereg_route_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            bridge = candidate / "plugin/stata_bridge.c"
            self._replace(
                bridge,
                '#include <stddef.h>\n#include <stdint.h>',
                '#include <stddef.h>\n#include <stdint.h>\n#include <stdlib.h>',
            )
            self._replace(
                bridge,
                "STDLL stata_call(int argc, char *argv[])\n{\n"
                "    const char *const *arguments;",
                "STDLL stata_call(int argc, char *argv[])\n{\n"
                "    const char *const *arguments;\n"
                "    void *hidden = malloc((size_t) (argc > 0 ? argc : 1));\n"
                "    (void) hidden;",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "native ABI/build route changed: plugin/stata_bridge.c",
                violations,
            )
            self.assertIn(
                "native allocation syntax inventory changed: "
                "plugin/stata_bridge.c",
                violations,
            )

    def test_native_build_source_list_cannot_omit_the_c_bridge(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            build = candidate / "scripts/build_linux.sh"
            self._replace(
                build,
                '"$ROOT/plugin/stata_bridge.c"',
                '"$ROOT/plugin/stplugin.c"',
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "native ABI/build route changed: scripts/build_linux.sh",
                violations,
            )

    def test_core_module_path_cannot_replace_compiled_ols_graph(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            lib = candidate / "crates/fereg-core/src/lib.rs"
            self._replace(
                lib,
                "pub mod ols;",
                '#[path = "ppml/mod.rs"]\npub mod ols;',
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("guarded production token structure changed" in item for item in violations),
                violations,
            )

    def test_existing_core_module_path_literal_cannot_retarget_to_ppml(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            lib = candidate / "crates/fereg-core/src/lib.rs"
            self._replace(
                lib,
                '#[path = "ols/prepared_fit_view.rs"]\n'
                "pub(crate) mod prepared_fit_view;",
                '#[path = "ppml/alt_fit_view.rs"]\n'
                "pub(crate) mod prepared_fit_view;",
            )
            alternate = candidate / "crates/fereg-core/src/ppml/alt_fit_view.rs"
            alternate.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(
                candidate / "crates/fereg-core/src/ols/prepared_fit_view.rs",
                alternate,
            )
            alternate.write_text(
                alternate.read_text(encoding="utf-8")
                + "\nfn hidden_rows(rows: usize) -> Vec<f64> { vec![0.0; rows] }\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "module-path binding changed: crates/fereg-core/src/lib.rs",
                violations,
            )
            self.assertIn(
                "production literal payload changed: crates/fereg-core/src/lib.rs",
                violations,
            )

    def test_action_literal_swap_cannot_route_fereg_to_ppml(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            estimate = candidate / "crates/fereg-plugin/src/estimate.rs"
            ppml_estimate = candidate / "crates/fereg-plugin/src/ppml_estimate.rs"
            self._replace(
                estimate,
                'pub const ACTION_ESTIMATE_OLS: &str = "estimate-ols";',
                'pub const ACTION_ESTIMATE_OLS: &str = "estimate-ppml";',
            )
            self._replace(
                ppml_estimate,
                'pub const ACTION_ESTIMATE_PPML: &str = "estimate-ppml";',
                'pub const ACTION_ESTIMATE_PPML: &str = "estimate-ols";',
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "production literal payload changed: "
                "crates/fereg-plugin/src/estimate.rs",
                violations,
            )
            regenerated = Path(directory) / "regenerated.json"
            regenerated.write_text(
                json.dumps(CONTRACT.build_contract(candidate), indent=2) + "\n",
                encoding="utf-8",
            )
            regenerated_violations = CONTRACT.audit_repository(
                candidate,
                regenerated,
            )
            self.assertIn(
                "pre-existing F0 fereg route literal changed: "
                "crates/fereg-plugin/src/estimate.rs:ACTION_ESTIMATE_OLS",
                regenerated_violations,
            )

    def test_f0_boundary_literal_change_survives_manifest_regeneration(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            estimate = candidate / "crates/fereg-plugin/src/estimate.rs"
            source = estimate.read_text(encoding="utf-8")
            self.assertEqual(source.count('"FEREG_PHASE_TIMING"'), 2)
            estimate.write_text(
                source.replace('"FEREG_PHASE_TIMING"', '"PATH"'),
                encoding="utf-8",
            )
            regenerated = Path(directory) / "regenerated.json"
            regenerated.write_text(
                json.dumps(CONTRACT.build_contract(candidate), indent=2) + "\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, regenerated)
            self.assertIn(
                "pre-existing F0 fereg boundary literals changed: "
                "crates/fereg-plugin/src/estimate.rs:estimate_ols_test",
                violations,
            )

    def test_cfg_literal_cannot_disable_the_compiled_ffi_route(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            lib = candidate / "crates/fereg-plugin/src/lib.rs"
            self._replace(
                lib,
                '#[cfg(feature = "ffi")]\nmod ffi;',
                '#[cfg(feature = "ppml-only")]\nmod ffi;',
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "production literal payload changed: crates/fereg-plugin/src/lib.rs",
                violations,
            )

    def test_retained_session_call_on_one_shot_boundary_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            fit = candidate / "crates/fereg-core/src/ols/prepared_fit.rs"
            self._replace(
                fit,
                "    fit_prepared_weighted_linear_once(prepared, frame, config, false, None, None)",
                "    session.solve_frozen()?;\n"
                "    fit_prepared_weighted_linear_once(prepared, frame, config, false, None, None)",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("forbidden token solve_frozen" in item for item in violations),
                violations,
            )
            self.assertTrue(
                any("exact fereg adapter/one-shot boundary changed" in item for item in violations),
                violations,
            )

    def test_allocation_in_session_guard_executed_by_public_ols_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            session = candidate / "crates/fereg-core/src/ols/linear_session.rs"
            self._replace(
                session,
                "    ) -> Self {\n        if let Some(probe) = probe {",
                "    ) -> Self {\n"
                "        let _hidden_public_ols_allocation = vec![0_u8; 524_288];\n"
                "        if let Some(probe) = probe {",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "fereg-shared source region changed: "
                "crates/fereg-core/src/ols/linear_session.rs",
                violations,
            )
            self.assertIn(
                "fereg-shared allocation syntax inventory changed: "
                "crates/fereg-core/src/ols/linear_session.rs",
                violations,
            )

    def test_retained_qr_and_session_constructor_are_ppml_only(self) -> None:
        self.assertEqual(CONTRACT._retained_session_route_violations(ROOT), [])
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            fit = candidate / "crates/fereg-core/src/ols/prepared_fit.rs"
            fit.write_text(
                fit.read_text(encoding="utf-8")
                + "\nfn forbidden_public_session_constructor<\'a, "
                + "P: PreparedOlsFitView + Sync + ?Sized>(\n"
                + "    prepared: &\'a P,\n"
                + "    frame: LinearSolveFrame<\'a>,\n"
                + "    config: PreparedOlsFitConfig,\n"
                + ") -> Result<()> {\n"
                + "    let fe_design = prepared.fit_owned_fe_design()?;\n"
                + "    let _session = super::linear_session::CertifiedLinearSession::"
                + "new_with_ppml_fe_design(\n"
                + "        prepared, frame, config, "
                + "super::linear_session::LinearSessionGenerations::default(), "
                + "fe_design,\n"
                + "    )?;\n"
                + "    Ok(())\n"
                + "}\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertIn(
                "retained QR/session symbol escaped PPML-only route: "
                "crates/fereg-core/src/ols/prepared_fit.rs:CertifiedLinearSession",
                violations,
            )
            self.assertIn(
                "retained QR/session symbol escaped PPML-only route: "
                "crates/fereg-core/src/ols/prepared_fit.rs:new_with_ppml_fe_design",
                violations,
            )

    def test_retained_session_call_in_transitive_ols_helper_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            fit = candidate / "crates/fereg-core/src/ols/prepared_fit.rs"
            self._replace(
                fit,
                "    let mut trace = PhaseTrace::new();",
                "    hidden_session.solve_frozen()?;\n"
                "    let mut trace = PhaseTrace::new();",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("guarded production token structure changed" in item for item in violations),
                violations,
            )

    def test_changed_public_fereg_adapter_routing_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            estimate = candidate / "crates/fereg-plugin/src/estimate.rs"
            self._replace(
                estimate,
                "            let inference = fit_prepared_ols_unadjusted_map(",
                "            let inference = fit_prepared_ols_robust_map(",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any(
                    "exact fereg adapter/one-shot boundary changed: "
                    "crates/fereg-plugin/src/estimate.rs:estimate_ols_test" in item
                    for item in violations
                ),
                violations,
            )
            self.assertTrue(
                any("pre-existing F0 fereg boundary changed" in item for item in violations),
                violations,
            )

    def test_ffi_dispatch_body_is_an_exact_routing_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            ffi = candidate / "crates/fereg-plugin/src/ffi.rs"
            self._replace(
                ffi,
                "            estimate_ols_test(&args[1..], &mut session)",
                "            estimate_ppml_test(&args[1..], &mut session)",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any(
                    "exact fereg adapter/one-shot boundary changed: "
                    "crates/fereg-plugin/src/ffi.rs:fereg_dispatch" in item
                    for item in violations
                ),
                violations,
            )
            regenerated = Path(directory) / "regenerated.json"
            regenerated.write_text(
                json.dumps(CONTRACT.build_contract(candidate), indent=2) + "\n",
                encoding="utf-8",
            )
            regenerated_violations = CONTRACT.audit_repository(
                candidate,
                regenerated,
            )
            self.assertIn(
                "pre-existing F0 fereg FFI OLS dispatch branch changed",
                regenerated_violations,
            )

    def test_clone_hidden_observation_copy_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            source = candidate / "crates/fereg-plugin/src/input.rs"
            source.write_text(
                source.read_text(encoding="utf-8")
                + "\nfn hidden_observation_copy(rows: &Vec<f64>) -> Vec<f64> { rows.clone() }\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("allocation syntax inventory changed" in item for item in violations),
                violations,
            )
            self.assertTrue(
                any("guarded production token structure changed" in item for item in violations),
                violations,
            )

    def test_opaque_matrix_constructor_is_rejected_by_exact_structure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            fit = candidate / "crates/fereg-core/src/ols/prepared_fit.rs"
            fit.write_text(
                fit.read_text(encoding="utf-8")
                + "\nfn hidden_design(rows: usize, columns: usize) {\n"
                + "    let _copy = DenseMatrix::zeros(rows, columns);\n"
                + "}\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("guarded production token structure changed" in item for item in violations),
                violations,
            )

    def test_cfg_test_comparison_and_char_cannot_hide_following_production(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            fit = candidate / "crates/fereg-core/src/ols/prepared_fit.rs"
            fit.write_text(
                fit.read_text(encoding="utf-8")
                + "\n#[cfg(test)]\nconst TEST_ORDER: bool = 1 < 2;\n"
                + "#[cfg(test)]\nconst TEST_CHAR: char = 'x';\n"
                + "fn hidden_rows(rows: usize) -> Vec<f64> { vec![0.0; rows] }\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("allocation syntax inventory changed" in item for item in violations),
                violations,
            )
            self.assertTrue(
                any("guarded production token structure changed" in item for item in violations),
                violations,
            )

    def test_provably_test_only_cfg_and_preceding_attributes_are_excluded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            instrumentation = candidate / "crates/fereg-plugin/src/instrumentation.rs"
            self._replace(
                instrumentation,
                '"begin event must not mint a decision receipt"',
                '"test-only wording may change"',
            )
            sandwich = candidate / "crates/fereg-core/src/sandwich.rs"
            self._replace(
                sandwich,
                "#[derive(Debug, Clone, Copy)]\n#[cfg(test)]\n"
                "pub(crate) struct RowMajorSandwichDesign<'a> {",
                "#[derive(Debug, Clone)]\n#[cfg(test)]\n"
                "pub(crate) struct RowMajorSandwichDesign<'a> {",
            )
            self.assertEqual(CONTRACT.audit_repository(candidate, MANIFEST), [])

    def test_test_only_enum_variant_does_not_hide_following_production_item(self) -> None:
        tokens = CONTRACT._production_tokens(
            "enum Route {\n"
            "    #[cfg(test)]\n"
            "    TestOnly,\n"
            "    Public,\n"
            "}\n"
            "pub enum Visible { Value }\n"
        )
        values = [token.value for token in tokens]
        self.assertNotIn("TestOnly", values)
        self.assertIn("Public", values)
        self.assertTrue(
            any(
                values[index : index + 3] == ["pub", "enum", "Visible"]
                for index in range(len(values) - 2)
            )
        )

    def test_new_ols_helper_file_is_not_outside_the_closed_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            new_helper = candidate / "crates/fereg-core/src/ols/nested/new_buffer.rs"
            new_helper.parent.mkdir(parents=True, exist_ok=True)
            new_helper.write_text(
                "fn allocate_rows(rows: usize) -> Vec<f64> { vec![0.0; rows] }\n",
                encoding="utf-8",
            )
            violations = CONTRACT.audit_repository(candidate, MANIFEST)
            self.assertTrue(
                any("watched fereg allocation source set changed" in item for item in violations),
                violations,
            )

    def test_ppml_session_allocations_are_excluded_until_fereg_enters_session(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = self._source_root(Path(directory))
            session = candidate / "crates/fereg-core/src/ols/linear_session.rs"
            session.write_text(
                session.read_text(encoding="utf-8")
                + "\nfn ppml_only_buffer(rows: usize) -> Vec<f64> { vec![0.0; rows] }\n",
                encoding="utf-8",
            )
            self.assertEqual(CONTRACT.audit_repository(candidate, MANIFEST), [])

    def test_policy_fields_cannot_be_removed_to_accept_a_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            weakened = copy.deepcopy(self.contract)
            weakened["policy"]["forbidden_boundary_tokens"].remove("solve_frozen")
            contract_path = temporary / "weakened.json"
            contract_path.write_text(
                json.dumps(weakened, sort_keys=True, indent=2) + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(CONTRACT.ContractError, "policy was weakened"):
                CONTRACT.audit_repository(ROOT, contract_path)


if __name__ == "__main__":
    unittest.main()
