from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/validate-final-sample-rebase.yml"


class FinalSampleRebaseValidationContractTests(unittest.TestCase):
    def test_validation_is_private_branch_and_mac_runner_scoped(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("codex/categorical-interactions", text)
        self.assertIn("self-hosted, macOS, ARM64, stata-mp, stfixest", text)
        self.assertIn("github.repository == 'johannes-schmieder/StFixest'", text)
        self.assertNotIn("pull_request", text)
        self.assertNotIn("ubuntu-latest", text)
        self.assertNotIn("windows-latest", text)

    def test_validation_applies_but_never_commits_generated_source(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("activate_final_sample_rebase.py", text)
        self.assertIn('"activation_committed": False', text)
        self.assertIn("subprocess.check_output", text)
        self.assertIn('["git", "diff", "--name-only", "--"]', text)
        self.assertIn("staged-validation receipt claims activation", text)
        self.assertNotIn("git add -A", text)
        self.assertNotIn("ols: activate final-sample", text)

    def test_validation_runs_every_required_pinned_rust_gate(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        for command in (
            "cargo fmt --all -- --check",
            "cargo clippy --workspace --all-targets --locked -- -D warnings",
            "cargo test --workspace --all-targets --locked\n",
            "cargo test --workspace --all-targets --locked --release",
            "cargo build --workspace --locked --release",
        ):
            self.assertIn(command, text)
        self.assertIn("/opt/homebrew/bin/rustup run 1.85.1 rustc --print sysroot", text)

    def test_validation_publishes_one_exact_sha_sealed_receipt(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            ".ci/staged-validation/final-sample-rebase/$GITHUB_SHA.json", text
        )
        self.assertIn('"tested_sha": os.environ["GITHUB_SHA"]', text)
        self.assertIn("payload_sha256", text)
        self.assertIn("generated_diff_sha256", text)
        self.assertIn("[skip ci]", text)


if __name__ == "__main__":
    unittest.main()
