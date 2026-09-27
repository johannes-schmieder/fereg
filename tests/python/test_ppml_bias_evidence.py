"""Scientific receipt binding and failure-denominator summary checks."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from ppml_bias_mc_analyze import statistics, validate_receipt
from ppml_bias_benchmark_analyze import validate_receipt as validate_benchmark


class BiasEvidenceTests(unittest.TestCase):
    def test_benchmark_binds_measurement_settings_and_artifact(self):
        settings = dict(rows=2880, levels_each=8, units=240, probes=256,
                        threads=1, memory="auto")
        plan = dict(warm_calls=5, refit_reference=True, plugin_sha256="plugin",
                    script_sha256="script", mata_sha256="mata", require_monitor=True,
                    monitor_sha256="monitor")
        measured = dict(rows=2880, levels=8, units=240, probes=256, threads=1,
                        memory="auto", engine="native", warm=5, refit_reference=True)
        source = {"files": {"a": "digest"}}
        receipt = dict(source=source, settings=measured, passed=True,
                       plugin_sha256="plugin", script_sha256="script", mata_sha256="mata",
                       timing_eligible=True, monitor={"sha256": "monitor"})
        validate_benchmark(plan, settings, "native", source, receipt)
        for field in measured:
            bad = copy.deepcopy(receipt)
            bad["settings"][field] = "changed"
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_benchmark(plan, settings, "native", source, bad)
        for field in ("source", "plugin_sha256", "script_sha256", "mata_sha256", "monitor"):
            bad = copy.deepcopy(receipt)
            bad[field] = {"sha256": "changed"} if field == "monitor" else "changed"
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_benchmark(plan, settings, "native", source, bad)

    def test_bias_precision_uses_draw_errors(self):
        result = statistics([1., 2., 3., float("nan")])
        self.assertEqual(result["n"], 3)
        self.assertEqual(result["bias"], 2.)
        self.assertAlmostEqual(result["bias_mcse"], 1/3**.5)
        self.assertAlmostEqual(result["rmse"], (14/3)**.5)
        self.assertEqual(statistics([float("nan")]), {"n": 0})

    def test_source_binary_adapter_and_draw_range_are_bound(self):
        source = {"head": "base", "manifest_sha256": "snapshot", "files": {"a": "digest"}}
        job = dict(engine="native", scenario="fixed_240", first=10001, last=10020,
                   method="probe", probes=256)
        identities = dict(plugin_sha256="plugin", adapter_manifest_sha256="adapter",
                          source_manifest_sha256=hashlib.sha256(
                              (json.dumps(source, indent=2, sort_keys=True)+"\n").encode()).hexdigest())
        receipt = dict(job, source=source, plugin_sha256="plugin", adapter_manifest_sha256="adapter")
        validate_receipt(dict(identities=identities), job, receipt)
        for field in (*job, "plugin_sha256", "adapter_manifest_sha256", "source"):
            bad = copy.deepcopy(receipt)
            bad[field] = {"head": "changed"} if field == "source" else "changed"
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_receipt(dict(identities=identities), job, bad)


if __name__ == "__main__":
    unittest.main()
