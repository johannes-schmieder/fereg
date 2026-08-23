from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESOLVER = ROOT / "scripts/materialize_bundled_reference_root.py"
PROBE_WORKFLOW = ROOT / ".github/workflows/fe-interaction-reference-probe.yml"


class BundledReferenceRootContractTests(unittest.TestCase):
    def test_resolver_is_offline_finite_and_non_enumerating(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        for token in (
            "repository_candidates",
            "_find_unique_local_store",
            "LOCAL_COMPONENTS = (\"reghdfe\", \"ftools\")",
            'BUNDLED_COMPONENT = "require"',
            "ci/reference-bundles/stata-require",
        ):
            self.assertIn(token, text)
        for forbidden in (
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
            "_locked_file_hashes",
            "bundle_sha256",
            "_sha256(bundle_path)",
            '"capture_network_policy": "offline-only"',
            '"bundle", "verify"',
            '"bundle", "list-heads"',
            'f"{commit}^{{commit}}"',
            '"fsck", "--full", "--strict"',
            '"status", "--porcelain=v1", "--untracked-files=all"',
            "preflight_reference_root(target, lock, ROOT)",
        ):
            self.assertIn(token, text)

    def test_large_local_dependencies_are_copy_isolated(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        for token in (
            '"clone"',
            '"--local"',
            '"--no-hardlinks"',
            '"--no-checkout"',
            '"protocol.file.allow=always"',
            '"checkout", "--detach", "--quiet", commit',
            '"remote", "set-url", "origin"',
        ):
            self.assertIn(token, text)
        self.assertNotIn("--hardlinks", text)

    def test_only_private_path_and_redacted_counts_are_emitted(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        self.assertIn("write_private_path(args.output, selected)", text)
        self.assertIn("reghdfe_object_stores=", text)
        self.assertIn("ftools_object_stores=", text)
        self.assertIn("bundle_present=", text)
        self.assertNotIn("print(selected", text)
        self.assertNotIn("print(candidate", text)
        self.assertNotIn("repr(candidate", text)
        self.assertNotIn("repr(selected", text)

    def test_probe_workflow_must_compile_and_attempt_bundled_resolver(self) -> None:
        text = PROBE_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("scripts/materialize_bundled_reference_root.py", text)
        self.assertIn("tests/python/test_bundled_reference_root_contract.py", text)
        self.assertIn("ci/reference-bundles/stata-require/**", text)
        self.assertIn("BUNDLED_REFERENCE_OUTCOME", text)
        self.assertIn("scripts/resolve_reference_root.py", text)
        self.assertNotIn("echo $STFIXEST_REFERENCE_ROOT", text)
        self.assertNotIn("cat $REFERENCE_ROOT_FILE", text)


if __name__ == "__main__":
    unittest.main()
