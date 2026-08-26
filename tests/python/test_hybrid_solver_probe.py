from __future__ import annotations

import copy
from pathlib import Path
import tempfile
import unittest

from scripts.hybrid_solver_probe import (
    ProbeError,
    _paired_comparison,
    _parse_timing,
    _timing_do,
    expand_cases,
    load_probe_manifest,
)


class HybridSolverProbeTests(unittest.TestCase):
    def test_manifest_is_development_only_and_expands_the_bounded_ladder(self) -> None:
        manifest = load_probe_manifest()
        self.assertFalse(manifest["qualification_eligible"])
        expanded = expand_cases(manifest)
        self.assertEqual(len(expanded), 5)
        self.assertEqual(
            {item["case_id"] for item in expanded},
            {
                "smoke_standard_n100k",
                "route_standard_n1m",
                "route_lowmob_n1m",
            },
        )
        self.assertEqual(
            {item["threads"] for item in expanded if item["scale"]["id"] == "n1m"},
            {1, 8},
        )

    def test_manifest_rejects_release_qualification(self) -> None:
        manifest = load_probe_manifest()
        mutated = copy.deepcopy(manifest)
        mutated["qualification_eligible"] = True
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "probe.json"
            import json

            path.write_text(json.dumps(mutated), encoding="utf-8")
            with self.assertRaises(ProbeError):
                load_probe_manifest(path)

    def test_timing_program_is_plugin_isolated_and_captures_handoff_diagnostics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "timing.do"
            _timing_do(
                path,
                Path("/isolated/prototype"),
                Path("/shared/data.dta"),
                "route_t8__prototype",
                "worker firm",
                8,
                1e-8,
                16_000,
                4,
            )
            source = path.read_text(encoding="utf-8")
        self.assertIn('adopath ++ "/isolated/prototype"', source)
        self.assertNotIn('adopath ++ "' + str(Path.cwd() / "dist") + '"', source)
        self.assertIn("__hyb_solver[1,12]", source)
        self.assertIn("__hyb_solver[1,13]", source)
        self.assertIn("__hyb_solver[1,14]", source)
        self.assertIn("__hyb_solver[1,16]", source)

    def test_timing_parser_preserves_polish_and_preconditioner_fields(self) -> None:
        staged = Path("/isolated/prototype/fereg_macos.plugin")
        lines = [f"PLUGIN_PATH|path={staged}"]
        for repetition, seconds in enumerate((1.2, 0.9, 1.0, 1.1), start=1):
            kind = "cold" if repetition == 1 else "warm"
            lines.extend(
                [
                    f"TIME_A|id=r|rep={repetition}|kind={kind}|rc=0|seconds={seconds}",
                    f"TIME_S|id=r|rep={repetition}|b1=.2|b2=-.1|v11=.01|v12=0|v22=.02",
                    f"TIME_B|id=r|rep={repetition}|requested=1|used=4|iters=3|stop=2",
                    f"TIME_F|id=r|rep={repetition}|absinf=1e-9|relinf=1e-10|rel2=1e-11|neq=1e-12",
                    f"TIME_C|id=r|rep={repetition}|setup=.2|solve=.3|fallback=0|warnings=1",
                    f"TIME_D|id=r|rep={repetition}|requested_precond=1|used_precond=4|pilot_iters=4",
                    f"TIME_E|id=r|rep={repetition}|pilot_contract=.02|native=.6|precommit=.7",
                ]
            )
        parsed = _parse_timing("\n".join(lines), "r", 4, staged)
        self.assertEqual(parsed["warm_median_seconds"], 1.0)
        solver = parsed["repetitions"][-1]["solver"]
        self.assertEqual(solver["iters"], 3)
        self.assertEqual(solver["used_precond"], 4)
        self.assertEqual(solver["warnings"], 1)

    def test_pair_comparison_requires_observed_route_and_records_resource_ratios(self) -> None:
        run = {
            "id": "route_t8",
            "case_id": "route",
            "threads": 8,
            "scale": {"workers": 100_000, "firms": 5_000},
        }
        rules = load_probe_manifest()["selection_rules"]

        def result(seconds: float, rss: int, used_precond: float, warnings: float):
            solver = {
                "requested": 1.0,
                "used": 4.0,
                "pilot_contract": 0.02,
                "used_precond": used_precond,
                "warnings": warnings,
            }
            return {
                "timing": {
                    "warm_median_seconds": seconds,
                    "repetitions": [{"solver": solver}],
                },
                "peak_rss_bytes": rss,
                "preflight": {
                    "coefficients": [0.2, -0.1],
                    "vce": [0.01, 0.0, 0.02],
                },
            }

        comparison = _paired_comparison(
            run,
            result(10.0, 1_000, 0.0, 0.0),
            result(8.0, 1_100, 4.0, 1.0),
            rules,
        )
        self.assertTrue(comparison["route_applicable"])
        self.assertTrue(comparison["route_observed"])
        self.assertEqual(comparison["prototype_over_baseline_warm_runtime"], 0.8)
        self.assertEqual(comparison["prototype_over_baseline_peak_rss"], 1.1)


if __name__ == "__main__":
    unittest.main()
