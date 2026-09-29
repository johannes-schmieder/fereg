import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from worker_diagnostics import failure_evidence


class WorkerDiagnosticsTests(unittest.TestCase):
    def test_quiet_application_failure_keeps_stage_and_logs(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "worker.log").write_text('. display "SYSTEMATIC_STAGE warmup"\nSYSTEMATIC_STAGE estimation\n')
            (root / "DONE").touch()
            evidence = failure_evidence(root, {"rc": 459}, "worker_failure")
            self.assertEqual(evidence["stage"], "estimation")
            self.assertEqual(evidence["application_rc"], 459)
            self.assertEqual(evidence["logs"], ["worker.log"])

    def test_metadata_is_authoritative_and_pass_has_no_failure(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "stdout.txt").write_text("SYSTEMATIC_STAGE estimation\n")
            meta = {"rc": 1, "error_stage": "extraction", "error": "missing covariance"}
            self.assertEqual(failure_evidence(root, meta, "worker_failure")["stage"], "extraction")
            self.assertIsNone(failure_evidence(root, {}, "pass"))

    def test_timeout_phase_does_not_require_metadata(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.assertEqual(failure_evidence(root, {}, "startup_timeout")["stage"], "initialization")
            (root / "START").touch()
            self.assertEqual(failure_evidence(root, {}, "command_timeout")["stage"], "estimation")
            (root / "DONE").touch()
            self.assertEqual(failure_evidence(root, {}, "extraction_timeout")["stage"], "extraction")

    def test_python_warmup_exception_writes_receipt_before_target(self):
        with tempfile.TemporaryDirectory() as folder:
            spec = importlib.util.spec_from_file_location("tested_worker", Path(__file__).with_name("python_worker.py"))
            worker = importlib.util.module_from_spec(spec)
            reference = Mock()
            reference.feols.side_effect = ValueError("Demeaning failed after 10000 iterations.")
            with patch.dict(sys.modules, {"pyfixest": reference}):
                spec.loader.exec_module(worker)
            config = dict(output=folder, cell=dict(model="ols", k=1, effects=["g1"]), phase="warm", warm="unused")
            with patch.object(worker.pd, "read_stata", return_value=object()):
                worker.run(config)
            meta = json.loads((Path(folder) / "metadata.json").read_text())
            self.assertEqual(meta["error_stage"], "warmup")
            self.assertEqual(meta["rc"], 1)
            self.assertIsNone(meta["seconds"])
            self.assertIn("10000 iterations", meta["error"])
            self.assertFalse((Path(folder) / "READY").exists())


if __name__ == "__main__":
    unittest.main()
