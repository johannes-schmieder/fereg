from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/ppml_solver_development.py"
MANIFEST = ROOT / "benchmarks/manifests/feppml_solver_development_v1.json"
DRIVER = ROOT / "tests/stata/development/feppml_solver_development.do"

SPEC = importlib.util.spec_from_file_location("ppml_solver_development", SCRIPT)
assert SPEC and SPEC.loader
DEVELOPMENT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEVELOPMENT)


class PpmlSolverDevelopmentTests(unittest.TestCase):
    def test_manifest_is_nonqualifying_and_has_the_frozen_cell_inventory(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        self.assertFalse(manifest["qualification_eligible"])
        self.assertEqual(manifest["request_wire"], "1.4")
        self.assertEqual(len(DEVELOPMENT.matrix_cells(manifest, "science_matrix")), 8)
        self.assertEqual(len(DEVELOPMENT.matrix_cells(manifest, "screening")), 48)
        self.assertEqual(len(DEVELOPMENT.matrix_cells(manifest, "scale_thread")), 32)
        self.assertEqual(len(DEVELOPMENT.matrix_cells(manifest, "all")), 88)
        self.assertEqual(manifest["candidate_routes"], list(DEVELOPMENT.ARMS[:-2]))
        self.assertIn(
            "joint correction warm start",
            manifest["fixed_policy"]["warm_solver_state_across_irls"],
        )
        self.assertEqual(manifest["science"]["vce_rtol"], 1e-6)

    def test_stata_missing_numeric_is_explicitly_nullable(self) -> None:
        self.assertIsNone(DEVELOPMENT.stata_optional_float("                    ."))
        self.assertIsNone(DEVELOPMENT.stata_optional_float(".a"))
        self.assertEqual(DEVELOPMENT.stata_optional_float(" 381 "), 381.0)

    def test_cell_filters_select_a_bounded_local_experiment(self) -> None:
        cells = DEVELOPMENT.matrix_cells(DEVELOPMENT.load_manifest(), "screening")
        selected = DEVELOPMENT.select_cells(
            cells,
            topology="balanced_2fe",
            controls=10,
            difficulty="easy",
            threads=1,
        )
        self.assertEqual(len(selected), 1)
        self.assertEqual(selected[0]["controls"], 10)
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.select_cells(cells, topology="missing_topology")

    def test_fused_cmg_skips_only_overwide_three_fe_nuisance_cells(self) -> None:
        cells = DEVELOPMENT.matrix_cells(DEVELOPMENT.load_manifest(), "screening")
        lookup = {
            (cell["topology"], cell["difficulty"]): cell
            for cell in cells
            if cell["controls"] == 10 and cell["threads"] == 1
        }
        self.assertTrue(DEVELOPMENT.arm_applicable("cmg_fused", lookup[("balanced_3fe", "easy")]))
        self.assertFalse(DEVELOPMENT.arm_applicable("cmg_fused", lookup[("balanced_3fe", "long")]))
        self.assertTrue(DEVELOPMENT.arm_applicable("cmg_fused", lookup[("chain_2fe", "long")]))
        self.assertTrue(DEVELOPMENT.arm_applicable("map", lookup[("balanced_3fe", "long")]))

    def test_driver_keeps_trace_untimed_and_uses_the_hidden_adapter(self) -> None:
        source = DRIVER.read_text(encoding="utf-8")
        self.assertIn("_fereg_estimate_ppml", source)
        self.assertIn("FEREG_PPML_SOLVER_TRACE", source)
        self.assertIn('if `"`trace\'"\' == "1"', source)
        self.assertIn("solvertrace iterate(100)", source)
        trace_exit = source.index("FEREG_PPML_SOLVER_TRACE_PASS")
        timer = source.index("timer on 91")
        self.assertLess(trace_exit, timer)
        self.assertIn("fesolver(`arm')", source)
        self.assertIn("fepreconditioner(schwarz)", source)
        self.assertIn("memory(fast)", source)
        self.assertIn("separation(none)", source)
        self.assertIn("assert y > 0", source)
        self.assertIn("rnormal()", source)
        self.assertIn("`repository'/dist", source)
        self.assertIn("FEREG_PPML_SOLVER_RUN_FAIL", source)
        self.assertIn("FEREG_PPML_SOLVER_REFERENCE_PASS", source)
        self.assertIn("FEREG_PPML_SOLVER_REFERENCE_ROUTE_PASS", source)
        self.assertIn('`"`arm\'"\' == "ppmlhdfe_lsmr"', source)
        self.assertIn(
            "separation(none) accel(lsmr)",
            source,
        )
        self.assertIn('local expected_acceleration "conjugate_gradient"', source)
        self.assertIn('local expected_acceleration "lsmr"', source)
        self.assertIn('local route_command `"`command\' verbose(2)"\'', source)
        self.assertIn("felsmrmode(`lsmr_mode')", source)
        self.assertIn("felsmrpolicy(`lsmr_policy')", source)
        self.assertIn("fecmgmode(fused)", source)
        self.assertIn("lftools.mlib", source)

    def test_runner_exposes_distinct_default_and_forced_lsmr_comparators(self) -> None:
        self.assertIn("ppmlhdfe", DEVELOPMENT.REFERENCE_ARMS)
        self.assertIn("ppmlhdfe_lsmr", DEVELOPMENT.REFERENCE_ARMS)
        self.assertIn("ppmlhdfe_lsmr", DEVELOPMENT.ARMS)
        self.assertIn("lsmr_fused_reuse", DEVELOPMENT.CANDIDATE_ARMS)
        self.assertIn("lsmr_joint_reuse", DEVELOPMENT.CANDIDATE_ARMS)
        self.assertIn("cmg_fused", DEVELOPMENT.CANDIDATE_ARMS)

    def test_analysis_fails_closed_without_complete_science_and_trace(self) -> None:
        analysis = DEVELOPMENT.analyze_document(
            {
                "records": [
                    {
                        "cell_id": "c",
                        "arm": "map",
                        "rc": 0,
                        "seconds": 1.0,
                        "sample_n": 10.0,
                        "ll": -2.0,
                        "deviance": 1.0,
                        "converged": 1.0,
                        "rank": 1.0,
                        "df_a": 0.0,
                        "df_resid": 9.0,
                        "b_names": ["x", "_cons"],
                        "b_values": [0.1, 0.2],
                        "v_names": ["x", "_cons"],
                        "v_values": [0.1, 0.0, 0.0, 0.1],
                    }
                ],
                "processes": [],
            }
        )
        self.assertIn("c:missing_oracle", analysis["science_failures"])
        self.assertIn("science_matrix:incomplete:0/8", analysis["science_failures"])
        self.assertEqual(analysis["route_recommendation"]["default"], "map")
        self.assertEqual(analysis["route_recommendation"]["regions"], [])
        self.assertTrue(
            all(
                value["label"] == "reject"
                for value in analysis["candidates"].values()
            )
        )

    def test_exclusive_json_refuses_to_overwrite_development_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            DEVELOPMENT.exclusive_json(path, {"qualification_eligible": False})
            with self.assertRaises(FileExistsError):
                DEVELOPMENT.exclusive_json(path, {"qualification_eligible": False})
            self.assertFalse(json.loads(path.read_text())["qualification_eligible"])

    def test_stata_status_requires_a_terminal_sentinel(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            console = Path(temporary) / "arm.log"
            console.write_text("", encoding="utf-8")
            self.assertEqual(
                DEVELOPMENT.stata_stage_status(console, 0, "PASS"),
                (999, "missing-success-sentinel"),
            )
            console.with_name("arm.stata.log").write_text(
                "FEREG_PPML_SOLVER_RUN_FAIL|arm=cg|rc=430\n",
                encoding="utf-8",
            )
            self.assertEqual(
                DEVELOPMENT.stata_stage_status(console, 0, "PASS"),
                (430, "failure-sentinel"),
            )

    def test_stata_status_does_not_treat_echoed_source_as_a_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            console = Path(temporary) / "arm.log"
            console.write_text(
                '. display as error "FEREG_PPML_SOLVER_RUN_FAIL|arm=map|rc=603"\n'
                "PASS\n",
                encoding="utf-8",
            )
            self.assertEqual(
                DEVELOPMENT.stata_stage_status(console, 0, "PASS"),
                (0, "success-sentinel"),
            )


if __name__ == "__main__":
    unittest.main()
