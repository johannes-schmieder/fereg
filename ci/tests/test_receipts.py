from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CI_DIR = Path(__file__).resolve().parents[1]
TESTED_SHA = "a" * 40


class ReceiptTests(unittest.TestCase):
    def build_receipt(
        self,
        *,
        process: dict[str, object],
        status: str | None,
        driver: str = "stata-harness",
        profile_summary: dict[str, object] | None = None,
        required_outputs: list[str] | None = None,
        output_files: list[str] | None = None,
    ) -> tuple[subprocess.CompletedProcess[str], dict[str, object]]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            process_path = root / "process.json"
            process_path.write_text(json.dumps(process), encoding="utf-8")
            status_path = root / "stata.status"
            if status is not None:
                status_path.write_text(status, encoding="utf-8")
            profile_summary_path = root / "profile-summary.json"
            if profile_summary is not None:
                profile_summary_path.write_text(
                    json.dumps(profile_summary), encoding="utf-8"
                )
            config_path = root / "profiles.json"
            config_path.write_text(
                json.dumps(
                    {
                        "profiles": {
                            "smoke": {
                                "driver": driver,
                                "suite": "ci/stata_version.do",
                                "required_outputs": required_outputs or [],
                            }
                        }
                    }
                ),
                encoding="utf-8",
            )
            for relative_name in output_files or []:
                output_path = root / relative_name
                output_path.parent.mkdir(parents=True, exist_ok=True)
                output_path.write_bytes(b"fixture output\n")
            receipt_path = root / "receipt.json"
            command = [
                sys.executable,
                str(CI_DIR / "make_stata_receipt.py"),
                "--profile-config",
                str(config_path),
                "--profile",
                "smoke",
                "--process-json",
                str(process_path),
                "--status-file",
                str(status_path),
                "--profile-summary",
                str(profile_summary_path),
                "--run-dir",
                str(root),
                "--receipt",
                str(receipt_path),
                "--tested-sha",
                TESTED_SHA,
                "--stata-executable",
                "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp",
            ]
            completed = subprocess.run(
                command, check=False, text=True, capture_output=True
            )
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            return completed, receipt

    @staticmethod
    def normal_process(**overrides: object) -> dict[str, object]:
        value: dict[str, object] = {
            "started_at": "2026-01-01T00:00:00Z",
            "completed_at": "2026-01-01T00:00:01Z",
            "duration_seconds": 1.0,
            "launch_error": None,
            "timed_out": False,
            "process_rc": 0,
        }
        value.update(overrides)
        return value

    @staticmethod
    def status(stata_rc: int) -> str:
        return (
            "schema_version=1\n"
            "profile=smoke\n"
            f"stata_rc={stata_rc}\n"
            "stata_version=18\n"
            "stata_edition=MP\n"
            "stata_os=Unix\n"
            "stata_machine_type=Mac (Apple Silicon)\n"
            "stata_processors=8\n"
            "tests_passed=1\n"
            "tests_failed=0\n"
            "completed=1\n"
        )

    def test_success(self) -> None:
        completed, receipt = self.build_receipt(
            process=self.normal_process(), status=self.status(0)
        )
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(receipt["status"], "success")
        self.assertEqual(receipt["stata_status"], "success")
        self.assertEqual(receipt["rust_status"], "not_run")
        self.assertIsNone(receipt["rust_rc"])
        self.assertIsNone(receipt["failure_kind"])
        self.assertEqual(receipt["tested_sha"], TESTED_SHA)

    def test_stata_error_wins_over_zero_process_rc(self) -> None:
        _, receipt = self.build_receipt(
            process=self.normal_process(process_rc=0), status=self.status(9)
        )
        self.assertEqual(receipt["status"], "failure")
        self.assertEqual(receipt["failure_kind"], "stata_error")
        self.assertEqual(receipt["stata_rc"], 9)

    def test_fixture_summary_preserves_failure_class(self) -> None:
        _, receipt = self.build_receipt(
            process=self.normal_process(process_rc=1),
            status=None,
            driver="fereg-fixtures",
            profile_summary={
                "completed": True,
                "status": "failure",
                "failure_kind": "missing_success_sentinel",
                "failure_detail": "missing terminal sentinel",
                "stata_rc": 0,
                "stata_version": "18",
                "stata_edition": "MP",
                "stata_os": "Unix",
                "stata_machine_type": "Mac (Apple Silicon)",
                "stata_processors": 8,
                "tests_passed": 2,
                "tests_failed": 1,
            },
        )
        self.assertEqual(receipt["status"], "failure")
        self.assertEqual(receipt["failure_kind"], "missing_success_sentinel")
        self.assertEqual(receipt["tests_passed"], 2)
        self.assertEqual(receipt["tests_failed"], 1)

    def test_launch_error(self) -> None:
        _, receipt = self.build_receipt(
            process=self.normal_process(
                process_rc=None, launch_error="FileNotFoundError: missing Stata"
            ),
            status=None,
        )
        self.assertEqual(receipt["failure_kind"], "launch_error")
        self.assertIsNone(receipt["stata_rc"])

    def test_timeout(self) -> None:
        _, receipt = self.build_receipt(
            process=self.normal_process(process_rc=-15, timed_out=True), status=None
        )
        self.assertEqual(receipt["failure_kind"], "timeout")

    def test_missing_status(self) -> None:
        _, receipt = self.build_receipt(process=self.normal_process(), status=None)
        self.assertEqual(receipt["failure_kind"], "missing_status")

    def test_crash_without_status(self) -> None:
        _, receipt = self.build_receipt(
            process=self.normal_process(process_rc=-9), status=None
        )
        self.assertEqual(receipt["failure_kind"], "crash")

    def test_missing_required_output(self) -> None:
        _, receipt = self.build_receipt(
            process=self.normal_process(),
            status=self.status(0),
            required_outputs=["output/result.txt"],
        )
        self.assertEqual(receipt["failure_kind"], "missing_result")

    def test_failed_profile_still_resolves_platform_output_name(self) -> None:
        completed, receipt = self.build_receipt(
            process=self.normal_process(process_rc=1),
            status=None,
            driver="fereg-fixtures",
            required_outputs=["plugin-evidence/{platform_plugin}"],
        )
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(receipt["status"], "failure")
        self.assertEqual(receipt["failure_kind"], "crash")
        expected = (
            "fereg_macos.plugin"
            if sys.platform == "darwin"
            else "fereg_linux.plugin"
        )
        self.assertEqual(
            receipt["required_outputs"],
            [{"path": f"plugin-evidence/{expected}", "present": False}],
        )

    def test_platform_plugin_output_is_resolved_from_profile_summary(self) -> None:
        completed, receipt = self.build_receipt(
            process=self.normal_process(),
            status=None,
            driver="fereg-fixtures",
            profile_summary={
                "completed": True,
                "status": "success",
                "failure_kind": None,
                "failure_detail": None,
                "stata_rc": 0,
                "stata_version": "18",
                "stata_edition": "MP",
                "stata_os": "Unix",
                "stata_machine_type": "PC (64-bit x86-64)",
                "stata_processors": 8,
                "tests_passed": 1,
                "tests_failed": 0,
                "plugin_filename": "fereg_linux.plugin",
            },
            required_outputs=["plugin-evidence/{platform_plugin}"],
            output_files=["plugin-evidence/fereg_linux.plugin"],
        )
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(receipt["status"], "success")
        self.assertEqual(
            receipt["required_outputs"],
            [
                {
                    "path": "plugin-evidence/fereg_linux.plugin",
                    "present": True,
                }
            ],
        )

    def test_rust_failure_makes_combined_push_receipt_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt_path = root / "receipt.json"
            receipt_path.write_text(
                json.dumps(
                    {
                        "status": "success",
                        "stata_status": "success",
                        "rust_status": "not_run",
                        "rust_rc": None,
                        "failure_kind": None,
                        "failure_detail": None,
                    }
                ),
                encoding="utf-8",
            )
            rust_status = root / "rust.status"
            rust_status.write_text(
                "schema_version=1\nrust_status=failure\nrust_rc=7\ncompleted=1\n",
                encoding="utf-8",
            )
            completed = subprocess.run(
                [
                    sys.executable,
                    str(CI_DIR / "augment_ci_receipt.py"),
                    str(receipt_path),
                    str(rust_status),
                ],
                check=False,
            )
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            self.assertEqual(completed.returncode, 0)
            self.assertEqual(receipt["status"], "failure")
            self.assertEqual(receipt["stata_status"], "success")
            self.assertEqual(receipt["rust_status"], "failure")
            self.assertEqual(receipt["rust_rc"], 7)
            self.assertEqual(receipt["failure_kind"], "rust_error")


if __name__ == "__main__":
    unittest.main()
