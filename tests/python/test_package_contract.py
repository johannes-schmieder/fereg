from __future__ import annotations

import hashlib
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MIN_NATIVE_PLUGIN_BYTES = 4096
NATIVE_PLUGIN_MAGICS = frozenset(
    {
        b"\x7fELF",
        b"MZ\x90\x00",
        b"\xfe\xed\xfa\xce",
        b"\xce\xfa\xed\xfe",
        b"\xfe\xed\xfa\xcf",
        b"\xcf\xfa\xed\xfe",
        b"\xca\xfe\xba\xbe",
        b"\xbe\xba\xfe\xca",
        b"\xca\xfe\xba\xbf",
        b"\xbf\xba\xfe\xca",
    }
)


def tracked_plugins(repository: Path) -> list[Path]:
    completed = subprocess.run(
        ["git", "-C", str(repository), "ls-files", "-z", "--", "*.plugin"],
        check=True,
        stdout=subprocess.PIPE,
    )
    relative_paths = [
        Path(raw.decode("utf-8"))
        for raw in completed.stdout.split(b"\0")
        if raw
    ]
    return [repository / relative for relative in relative_paths]


def invalid_native_plugins(paths: list[Path]) -> list[Path]:
    invalid: list[Path] = []
    for path in paths:
        if not path.is_file() or path.stat().st_size < MIN_NATIVE_PLUGIN_BYTES:
            invalid.append(path)
            continue
        with path.open("rb") as handle:
            if handle.read(4) not in NATIVE_PLUGIN_MAGICS:
                invalid.append(path)
    return invalid


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
        # Source snapshots are built from the Git index. Target-native build
        # outputs under dist/ are intentionally ignored and must not make this
        # source-tree contract non-reentrant after a successful local build.
        plugins = tracked_plugins(ROOT)
        invalid = [path.relative_to(ROOT) for path in invalid_native_plugins(plugins)]
        self.assertEqual(invalid, [])

    def test_ignored_build_output_is_not_treated_as_a_shipped_plugin(self):
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            subprocess.run(
                ["git", "init", "--quiet", str(repository)],
                check=True,
            )
            (repository / ".gitignore").write_text("/dist/*.plugin\n", encoding="utf-8")
            dist = repository / "dist"
            dist.mkdir()
            generated = dist / "fereg_macos.plugin"
            generated.write_bytes(b"\xca\xfe\xba\xbe" + b"native" * 1000)
            subprocess.run(
                ["git", "-C", str(repository), "add", ".gitignore"],
                check=True,
            )
            self.assertEqual(tracked_plugins(repository), [])

    def test_tracked_placeholder_plugin_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            subprocess.run(
                ["git", "init", "--quiet", str(repository)],
                check=True,
            )
            placeholder = repository / "fereg_linux.plugin"
            placeholder.write_text("placeholder, not a native binary\n", encoding="utf-8")
            subprocess.run(
                ["git", "-C", str(repository), "add", placeholder.name],
                check=True,
            )
            self.assertEqual(invalid_native_plugins(tracked_plugins(repository)), [placeholder])

    def test_fereg_is_public_and_feppml_remains_fail_closed(self):
        fereg = (ROOT / "stata/fereg.ado").read_text(encoding="utf-8").lower()
        package = (ROOT / "stata/fereg.pkg").read_text(encoding="utf-8")
        self.assertIn("_fereg_estimate_public `0'", fereg)
        self.assertIn("f _fereg_ols.ado", package)
        self.assertIn("f _fereg_estimate_ols.ado", package)
        self.assertNotIn("f _fereg_ols_hidden_numeric_v1.ado", package)
        self.assertNotIn("f _fereg_estimate_ols_test.ado", package)
        self.assertNotIn("f _fereg_prepare_ols_test.ado", package)

        feppml = (ROOT / "stata/feppml.ado").read_text(encoding="utf-8").lower()
        self.assertIn("exit 498", feppml)
        self.assertNotIn("plugin call", feppml)

    def test_legacy_stata_commands_are_not_shipped(self):
        legacy = ("st" "fixest", "st" "feols", "st" "fepois")
        shipped = {path.name for path in (ROOT / "stata").iterdir() if path.is_file()}
        for command in legacy:
            self.assertFalse(any(name == command or name.startswith(f"{command}.") for name in shipped))
        package = (ROOT / "stata/fereg.pkg").read_text(encoding="utf-8")
        for command in legacy:
            self.assertNotIn(command, package)


if __name__ == "__main__":
    unittest.main()
