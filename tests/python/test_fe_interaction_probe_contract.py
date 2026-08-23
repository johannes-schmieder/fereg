from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/fe_interaction_probe.py"
WORKFLOW = ROOT / ".github/workflows/fe-interaction-reference-probe.yml"


class FeInteractionProbeContractTests(unittest.TestCase):
    def test_probe_reuses_exact_isolated_reference_preflight(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        for token in (
            "preflight_reference_root",
            "stage_references",
            "_write_wrapper",
            "seal_payload",
            "raw_capture_committed",
        ):
            self.assertIn(token, text)
        self.assertIn('"preflight"', text)
        self.assertNotIn("urllib", text)
        self.assertNotIn("requests", text)

    def test_failed_probe_retains_private_debug_artifacts(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn('shutil.copy2(log_file, output_dir / "run.log")', text)
        self.assertIn('shutil.copy2(capture_file, output_dir / "capture.machine")', text)
        self.assertIn("baseline_sentinel_count", text)
        self.assertIn('"stage": "probe_execution"', text)
        self.assertIn('"workflow_run_id"', text)
        self.assertIn('"workflow_run_attempt"', text)

    def test_workflow_is_private_main_and_mac_runner_scoped(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("branches:\n      - main", text)
        self.assertNotIn("codex/categorical-interactions", text)
        self.assertIn("self-hosted, macOS, ARM64, stata-mp, stfixest", text)
        self.assertIn("github.repository == 'johannes-schmieder/StFixest'", text)
        self.assertNotIn("pull_request", text)
        self.assertNotIn("ubuntu-latest", text)
        self.assertNotIn("windows-latest", text)

    def test_every_started_probe_leaves_a_source_bound_summary(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("Initialize durable probe status", text)
        self.assertIn("Select Python with tomllib", text)
        self.assertIn("Finalize missing or malformed probe status", text)
        self.assertIn("python_tomllib_unavailable", text)
        self.assertIn("probe_process_failed_before_summary", text)
        self.assertIn("summary_missing_or_malformed", text)
        self.assertIn("workflow_run_id", text)
        self.assertIn("payload_sha256", text)
        self.assertIn("probe summary source SHA mismatch", text)

    def test_only_redacted_summary_is_committed(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(".ci/reference-probes/fe-interaction/$GITHUB_SHA.json", text)
        self.assertNotIn("git add capture.machine", text)
        self.assertNotIn("git add run.log", text)
        self.assertIn('"raw_capture_committed": False', text)


if __name__ == "__main__":
    unittest.main()
