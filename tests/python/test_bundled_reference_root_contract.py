from __future__ import annotations

import hashlib
import json
import subprocess
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESOLVER = ROOT / "scripts/materialize_bundled_reference_root.py"
PROBE_WORKFLOW = ROOT / ".github/workflows/fe-interaction-reference-probe.yml"
BUNDLE_ROOT = ROOT / "ci/reference-bundles/stata-require"
LOCK = ROOT / "references.lock.toml"
COMPONENTS = ("reghdfe", "ftools", "require")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


class BundledReferenceRootContractTests(unittest.TestCase):
    def test_resolver_is_offline_finite_and_non_enumerating(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        for token in (
            'COMPONENTS = ("reghdfe", "ftools", "require")',
            'BUNDLE_ROOT = ROOT / "ci/reference-bundles/stata-require"',
            "_bundle_inventory",
            "_validate_bundle_manifest",
            "_validate_bundle_heads",
            "_clone_bundle",
            "preflight_reference_root(target, lock, ROOT)",
        ):
            self.assertIn(token, text)
        for forbidden in (
            "repository_candidates",
            "_find_unique_local_store",
            ".rglob(",
            ".glob(",
            ".iterdir(",
            "os.listdir",
            "os.scandir",
            "os.walk",
            "urllib",
            "requests",
            "curl",
            '"fetch"',
            "git fetch",
        ):
            self.assertNotIn(forbidden, text)

    def test_bundle_identity_and_integrity_are_exactly_verified(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        for token in (
            "required_capture_files",
            "required_capture_sha256",
            "bundle_sha256",
            "_sha256(bundle_path)",
            '"capture_network_policy": "offline-only"',
            '"bundle", "list-heads"',
            'f"{commit}^{{commit}}"',
            '"fsck", "--full", "--strict"',
            '"status", "--porcelain=v1", "--untracked-files=all"',
            '"remote", "set-url", "origin"',
        ):
            self.assertIn(token, text)

    def test_repository_contains_all_lock_pinned_bundles(self) -> None:
        lock = tomllib.loads(LOCK.read_text(encoding="utf-8"))
        entries = {
            entry["name"]: entry
            for entry in lock["reference"]
            if entry.get("name") in COMPONENTS
        }
        self.assertEqual(set(entries), set(COMPONENTS))
        for component in COMPONENTS:
            entry = entries[component]
            commit = entry["commit"]
            directory = BUNDLE_ROOT / component / commit
            manifest_path = directory / "manifest.json"
            bundle_path = directory / f"{component}-{commit}.bundle"
            self.assertTrue(manifest_path.is_file(), component)
            self.assertTrue(bundle_path.is_file(), component)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(manifest["schema"], 2)
            self.assertEqual(manifest["status"], "success")
            self.assertEqual(manifest["component"], component)
            self.assertEqual(manifest["source_repository"], entry["repository"])
            self.assertEqual(manifest["source_commit"], commit)
            self.assertEqual(manifest["checkout_dir"], entry["checkout_dir"])
            self.assertEqual(manifest["required_files"], entry["required_capture_sha256"])
            self.assertEqual(manifest["capture_network_policy"], "offline-only")
            self.assertEqual(manifest["provisioning_mode"], "offline-local-reseal")
            self.assertEqual(manifest["provisioning_repository"], "johannes-schmieder/fereg")
            self.assertEqual(manifest["bundle_sha256"], sha256_file(bundle_path))
            completed = subprocess.run(
                ["git", "bundle", "list-heads", str(bundle_path)],
                cwd=ROOT,
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, component)
            self.assertEqual(
                completed.stdout.strip(),
                f"{commit} refs/heads/fereg-pin-{component}",
            )

    def test_only_private_path_and_redacted_counts_are_emitted(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        self.assertIn("write_private_path(args.output, selected)", text)
        self.assertIn("bundles_present=", text)
        self.assertIn("component=", text)
        self.assertNotIn("print(selected", text)
        self.assertNotIn("print(bundle", text)
        self.assertNotIn("print(destination", text)
        self.assertNotIn("repr(selected", text)



if __name__ == "__main__":
    unittest.main()
