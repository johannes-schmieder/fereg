from __future__ import annotations

import hashlib
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class PackageContractTests(unittest.TestCase):
    def test_governing_spec_hash_is_locked(self):
        with (ROOT / "references.lock.toml").open("rb") as handle:
            lock = tomllib.load(handle)
        expected = lock["governing_spec"]["sha256"]
        actual = hashlib.sha256((ROOT / "AGENTS.md").read_bytes()).hexdigest()
        self.assertEqual(actual, expected)

    def test_reference_commits_are_immutable_hashes(self):
        with (ROOT / "references.lock.toml").open("rb") as handle:
            lock = tomllib.load(handle)
        by_name = {entry["name"]: entry for entry in lock["reference"]}
        for name in ("pyfixest", "within", "reghdfe", "ppmlhdfe"):
            commit = by_name[name]["commit"]
            self.assertEqual(len(commit), 40)
            int(commit, 16)

    def test_spi_hashes_match_local_sources(self):
        with (ROOT / "references.lock.toml").open("rb") as handle:
            lock = tomllib.load(handle)
        spi = next(entry for entry in lock["reference"] if entry["name"] == "stata-spi")
        for filename, key in (
            ("stplugin.c", "local_stplugin_c_sha256"),
            ("stplugin.h", "local_stplugin_h_sha256"),
        ):
            actual = hashlib.sha256((ROOT / "plugin" / filename).read_bytes()).hexdigest()
            self.assertEqual(actual, spi[key])

    def test_toolchain_and_workspace_msrv_agree(self):
        with (ROOT / "Cargo.toml").open("rb") as handle:
            cargo = tomllib.load(handle)
        with (ROOT / "rust-toolchain.toml").open("rb") as handle:
            toolchain = tomllib.load(handle)
        self.assertEqual(cargo["workspace"]["package"]["rust-version"], "1.85")
        self.assertEqual(toolchain["toolchain"]["channel"], "1.85.1")

    def test_target_native_builds_load_the_final_plugin(self):
        scripts = {
            "build_linux.sh": "run_actual_unix_plugin_test.sh",
            "build_macos.sh": "run_actual_unix_plugin_test.sh",
            "build_windows.ps1": "run_actual_windows_plugin_test.ps1",
        }
        for script, harness in scripts.items():
            text = (ROOT / "scripts" / script).read_text(encoding="utf-8")
            self.assertIn(harness, text)

    def test_no_placeholder_native_binary_is_shipped(self):
        plugins = list((ROOT / "dist").glob("*.plugin"))
        self.assertEqual(plugins, [])

    def test_estimators_fail_closed(self):
        for filename in ("stfeols.ado", "stfepois.ado"):
            text = (ROOT / "stata" / filename).read_text(encoding="utf-8").lower()
            self.assertIn("exit 498", text)
            self.assertNotIn("plugin call", text)


if __name__ == "__main__":
    unittest.main()
