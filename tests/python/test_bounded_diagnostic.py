"""Safety boundaries for deliberately memory-restricted, untimed diagnostics."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import types
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("bounded_diagnose", ROOT / "benchmarks/performance/diagnose.py")
diagnose = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(diagnose)


class BoundedDiagnosticTest(unittest.TestCase):
    def test_inherited_override_is_removed(self):
        env = diagnose.bounded_environment({"cell": {"threads": 4}}, Path("/tmp/probe"),
            {"FEREG_MEMORY_LIMIT_BYTES": "999999", "OMP_NUM_THREADS": "28", "PATH": "kept"})
        self.assertNotIn("FEREG_MEMORY_LIMIT_BYTES", env)
        self.assertEqual(env["OMP_NUM_THREADS"], "4")
        self.assertEqual(env["PATH"], "kept")

    def test_memory_override_only_restricts_available_memory(self):
        case = {"cell": {"threads": 4}, "memory_limit_bytes": 800, "phase_timing": True}
        env = diagnose.bounded_environment(case, Path("/tmp/probe"), {}, 1000)
        self.assertEqual(env["FEREG_MEMORY_LIMIT_BYTES"], "800")
        self.assertEqual(env["FEREG_PHASE_TIMING"], "1")
        for limit in [1001, 0, -1, True, "800"]:
            with self.subTest(limit=limit), self.assertRaises(ValueError):
                diagnose.bounded_environment(dict(case, memory_limit_bytes=limit), Path("/tmp/probe"), {}, 1000)
        with self.assertRaises(ValueError):
            diagnose.bounded_environment(case, Path("/tmp/probe"), {})

    def test_available_memory_requires_units(self):
        with patch.object(Path, "read_text", return_value="MemTotal: 10000 kB\nMemAvailable: 5000 kB\n"):
            self.assertEqual(diagnose.available_linux_memory(), 5000 * 1024)
        with patch.object(Path, "read_text", return_value="MemAvailable: 5000 MB\n"):
            with self.assertRaises(ValueError):
                diagnose.available_linux_memory()

    def test_ordinary_completion_is_preserved(self):
        child = Mock()
        self.assertEqual(diagnose.wait_bounded(child, 1), (None, None))
        child.wait.assert_called_once_with(timeout=1)

    def test_timeout_kills_and_reaps_process_group(self):
        child = Mock(pid=123)
        child.wait.side_effect = [subprocess.TimeoutExpired("diagnostic", 1), -9]
        with patch.object(diagnose.os, "killpg") as kill:
            self.assertEqual(diagnose.wait_bounded(child, 1), ("process_timeout", None))
            kill.assert_called_once_with(123, diagnose.signal.SIGKILL)
        self.assertEqual(child.wait.call_count, 2)

    def test_rss_stop_includes_descendants(self):
        descendant = Mock()
        descendant.memory_info.return_value = types.SimpleNamespace(rss=70)
        parent = Mock()
        parent.children.return_value = [descendant]
        parent.memory_info.return_value = types.SimpleNamespace(rss=40)
        fake = types.SimpleNamespace(Process=Mock(return_value=parent), NoSuchProcess=ProcessLookupError)
        child = Mock(pid=123)
        child.poll.return_value = None
        with patch.dict(sys.modules, psutil=fake), patch.object(diagnose.os, "killpg") as kill:
            self.assertEqual(diagnose.wait_bounded(child, 1, 100), ("rss_limit", 110))
            kill.assert_called_once_with(123, diagnose.signal.SIGKILL)
        child.wait.assert_called_once()

    def test_monitor_error_still_reaps_process_group(self):
        parent = Mock()
        parent.children.side_effect = PermissionError("monitor unavailable")
        fake = types.SimpleNamespace(Process=Mock(return_value=parent), NoSuchProcess=ProcessLookupError)
        child = Mock(pid=123)
        child.poll.return_value = None
        with patch.dict(sys.modules, psutil=fake), patch.object(diagnose.os, "killpg") as kill:
            with self.assertRaises(PermissionError):
                diagnose.wait_bounded(child, 1, 100)
            kill.assert_called_once_with(123, diagnose.signal.SIGKILL)
        child.wait.assert_called_once()


if __name__ == "__main__":
    unittest.main()
