from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CORE = ROOT / "crates/fereg-core/src"
PLUGIN = ROOT / "crates/fereg-plugin/src"


class RustSourceContractTests(unittest.TestCase):
    def test_external_rust_dependencies_are_immutable_and_statically_linked(self):
        for manifest in (ROOT / "crates").glob("*/Cargo.toml"):
            with manifest.open("rb") as handle:
                parsed = tomllib.load(handle)
            for dependency, value in parsed.get("dependencies", {}).items():
                self.assertIsInstance(value, dict, f"{manifest}: {dependency}")
                if "path" in value:
                    continue
                if "git" in value:
                    revision = value.get("rev", "")
                    self.assertRegex(
                        revision,
                        r"^[0-9a-f]{40}$",
                        f"floating git dependency in {manifest}: {dependency}",
                    )
                    continue
                version = value.get("version", "")
                self.assertTrue(
                    version.startswith("="),
                    f"non-exact registry dependency in {manifest}: {dependency}",
                )

    def test_safe_core_forbids_unsafe_and_contains_no_unsafe_blocks(self):
        lib = (CORE / "lib.rs").read_text(encoding="utf-8")
        self.assertIn("#![forbid(unsafe_code)]", lib)
        for source in CORE.glob("*.rs"):
            text = source.read_text(encoding="utf-8")
            self.assertIsNone(re.search(r"\bunsafe\s*(?:\{|fn|impl|trait)", text), source)

    def test_unsafe_is_confined_to_narrow_ffi_module(self):
        unsafe_files = []
        for source in PLUGIN.glob("*.rs"):
            if re.search(r"\bunsafe\b", source.read_text(encoding="utf-8")):
                unsafe_files.append(source.name)
        self.assertEqual(unsafe_files, ["ffi.rs"])

    def test_ffi_boundary_catches_panics_and_validates_argv(self):
        text = (PLUGIN / "ffi.rs").read_text(encoding="utf-8")
        self.assertIn("catch_unwind", text)
        self.assertIn("argc < 0", text)
        self.assertIn("argv.is_null()", text)
        self.assertIn("pointer.is_null()", text)
        self.assertIn("CStr::from_ptr", text)
        self.assertIn("ErrorCode::Panic", text)

    def test_only_stata_entrypoints_are_exported_by_linker_controls(self):
        linux = (ROOT / "plugin/exports_linux.map").read_text(encoding="utf-8")
        macos = (ROOT / "plugin/exports_macos.txt").read_text(encoding="utf-8").splitlines()
        self.assertRegex(linux, r"global:\s*pginit;\s*stata_call;")
        self.assertIn("local:\n    *;", linux)
        self.assertEqual([line.strip() for line in macos if line.strip()], ["_pginit", "_stata_call"])

    def test_public_ols_uses_the_guarded_spi_path_and_ppml_stays_disabled(self):
        dispatch = (PLUGIN / "dispatch.rs").read_text(encoding="utf-8")
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        fereg = (ROOT / "stata/fereg.ado").read_text(encoding="utf-8")
        feppml = (ROOT / "stata/feppml.ado").read_text(encoding="utf-8")
        self.assertIn('"estimate-ppml"', dispatch)
        self.assertIn("ErrorCode::Unsupported", dispatch)
        self.assertIn("ACTION_ESTIMATE_OLS_TEST", estimate)
        self.assertIn("_fereg_estimate_public `0'", fereg)
        self.assertIn("exit 498", feppml)
        self.assertNotIn("plugin call", feppml)

    def test_final_native_builds_audit_then_load_before_hashing(self):
        expectations = {
            "build_linux.sh": ("audit_binary.py", "run_actual_unix_plugin_test.sh", "sha256sum"),
            "build_macos.sh": ("audit_binary.py", "run_actual_unix_plugin_test.sh", "shasum"),
            "build_windows.ps1": ("audit_binary.py", "run_actual_windows_plugin_test.ps1", "Get-FileHash"),
        }
        for filename, markers in expectations.items():
            text = (ROOT / "scripts" / filename).read_text(encoding="utf-8")
            positions = [text.index(marker) for marker in markers]
            self.assertEqual(positions, sorted(positions), filename)

    def test_no_owner_or_todo_repository_placeholder_remains(self):
        candidates = [ROOT / "Cargo.toml", *sorted((ROOT / "crates").glob("*/Cargo.toml"))]
        joined = "\n".join(path.read_text(encoding="utf-8") for path in candidates)
        self.assertNotIn("OWNER", joined)
        self.assertNotIn("example.com", joined)


if __name__ == "__main__":
    unittest.main()
