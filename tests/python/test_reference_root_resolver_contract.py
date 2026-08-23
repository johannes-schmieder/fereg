from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESOLVER = ROOT / "scripts/resolve_reference_root.py"
WORKFLOW = ROOT / ".github/workflows/fe-interaction-reference-probe.yml"


class ReferenceRootResolverContractTests(unittest.TestCase):
    def test_resolver_uses_a_finite_non_enumerating_allowlist(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        self.assertIn("CANDIDATE_SUFFIXES = (", text)
        self.assertIn('"Git"', text)
        self.assertIn("preflight_reference_root", text)
        self.assertIn("offline_reference_root_ambiguous", text)
        self.assertIn("explicit_reference_root_failed_preflight", text)
        for forbidden in (
            ".rglob(",
            ".glob(",
            "os.walk",
            "urllib",
            "requests",
            "git fetch",
        ):
            self.assertNotIn(forbidden, text)

    def test_materialization_is_local_only_pinned_and_copy_isolated(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        for token in (
            '"clone"',
            '"--local"',
            '"--no-hardlinks"',
            '"--no-checkout"',
            '"protocol.file.allow=always"',
            '"checkout", "--detach", "--quiet", commit',
            '"remote", "get-url", "origin"',
            '"rev-parse", "--verify", f"{commit}^{{commit}}"',
            "_github_repository_identity",
            "preflight_reference_root(target, lock, ROOT)",
        ):
            self.assertIn(token, text)
        for token in (
            '"GIT_CONFIG_NOSYSTEM": "1"',
            '"GIT_CONFIG_GLOBAL": os.devnull',
            '"GIT_TERMINAL_PROMPT": "0"',
        ):
            self.assertIn(token, text)
        self.assertNotIn('"fetch"', text)
        self.assertNotIn("--hardlinks", text)

    def test_resolver_writes_only_a_private_path_file(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        self.assertIn("stat.S_IRUSR | stat.S_IWUSR", text)
        self.assertIn("os.replace(temporary, output)", text)
        self.assertNotIn("print(candidate", text)
        self.assertNotIn("print(selected", text)
        self.assertNotIn("repr(candidate", text)
        self.assertIn("candidate_index={index}", text)
        self.assertIn("materialization_attempts=", text)

    def test_workflow_passes_the_root_without_logging_or_artifacting_it(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("Resolve offline pinned reference root", text)
        self.assertIn("scripts/resolve_reference_root.py --output", text)
        self.assertIn("--materialize-parent", text)
        self.assertIn('STFIXEST_REFERENCE_ROOT="$(cat "$REFERENCE_ROOT_FILE")"', text)
        self.assertIn("REFERENCE_ROOT_OUTCOME", text)
        self.assertIn("offline_reference_root_unresolved", text)
        self.assertNotIn("echo $STFIXEST_REFERENCE_ROOT", text)
        self.assertNotIn("cat $REFERENCE_ROOT_FILE", text)
        self.assertNotIn("stfixest-reference-root.path/", text)
        self.assertNotIn("stfixest-reference-materialized/", text)


if __name__ == "__main__":
    unittest.main()
