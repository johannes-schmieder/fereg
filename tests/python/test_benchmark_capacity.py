import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import ppml_bias_benchmark_campaign as campaign
from run_stata_tests import FixtureResult


class BenchmarkCapacityTests(unittest.TestCase):
    def test_later_unsupported_case_rejects_before_any_benchmark(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            plugin = root / "plugin"
            plugin.write_bytes(b"fixture")
            result = FixtureResult(root, 0, 198, "", "Stata r(198)", "application",
                                   {"stata_processors": "4"}, "")
            with patch.object(campaign, "run_fixture", return_value=result):
                with self.assertRaisesRegex(ValueError, "no timed fits"):
                    campaign.preflight_capacity("stata", root, root, plugin,
                                                [{"threads": 1}, {"threads": 8}])
            receipt = json.loads((root / "capacity.json").read_text())
            self.assertEqual(receipt["requested_threads"], 8)
            self.assertFalse(receipt["passed"])

    def test_capacity_requires_application_success_not_process_success(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            plugin = root / "plugin"
            plugin.write_bytes(b"fixture")
            for rc, error in [(None, None), (0, "missing terminal sentinel")]:
                result = FixtureResult(root, 0, rc, "", error, "application", {}, "")
                with patch.object(campaign, "run_fixture", return_value=result):
                    with self.assertRaises(ValueError):
                        campaign.preflight_capacity("stata", root, root, plugin, [{"threads": 4}])


if __name__ == "__main__":
    unittest.main()
