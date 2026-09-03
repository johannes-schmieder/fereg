from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

from scripts.feppml_public_state import ACTIVATED, public_state


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/stata/parity_ppmlhdfe"
TRANSACTION = ROOT / "stata/_fereg_estimate_ppml.ado"
PUBLIC = ROOT / "stata/feppml.ado"
BREAK = ROOT / "tests/licensed_stata/estimate_ppml_eclass_break.do"
BREAK_RUNNER = ROOT / "scripts/run_stata_break_test.py"
PLUGIN = ROOT / "crates/fereg-plugin/src/ppml_estimate.rs"

SHARED = {
    "feppml_command_contract.do": "estimate_ppml_hidden_command_contract.do",
    "feppml_saved_slopes.do": "estimate_ppml_hidden_saved_slopes.do",
    "feppml_time_series.do": "estimate_ppml_hidden_time_series.do",
}


class PpmlStataSurfaceCompletionTests(unittest.TestCase):
    def text(self, relative: str) -> str:
        return (ROOT / relative).read_text(encoding="utf-8")

    def test_new_science_fixtures_are_shared_across_public_states(self) -> None:
        for public_name, hidden_name in SHARED.items():
            with self.subTest(public_name=public_name):
                shared = (FIXTURES / public_name).read_text(encoding="utf-8")
                hidden = (FIXTURES / hidden_name).read_text(encoding="utf-8")
                self.assertIn(
                    'local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"\'',
                    shared,
                )
                self.assertIn('local feppml_test_command "feppml"', shared)
                self.assertNotIn("_fereg_estimate_ppml", shared)
                self.assertIn(
                    'global FEREG_PPML_TEST_COMMAND "_fereg_ppml"', hidden
                )
                self.assertIn(public_name, hidden)

        public = PUBLIC.read_text(encoding="utf-8")
        activated = public_state(ROOT) == ACTIVATED
        if activated:
            self.assertIn("_fereg_ppml `0'", public)
            self.assertNotIn("exit 498", public)
        else:
            self.assertIn("exit 498", public)
            self.assertNotIn("_fereg_ppml `0'", public)
        package = self.text("stata/fereg.pkg")
        stager = self.text("scripts/stage_release.py")
        for private in (
            "_fereg_ppml.ado",
            "_fereg_estimate_ppml.ado",
            "feppml_p.ado",
        ):
            assertion = self.assertIn if activated else self.assertNotIn
            assertion(f"f {private}", package)
            assertion(f'"{private}"', stager)

    def test_saved_intercept_and_slope_contract_is_complete(self) -> None:
        source = (FIXTURES / "feppml_saved_slopes.do").read_text(encoding="utf-8")
        required = (
            "absorb(__is_part=g##c.z h)",
            "__is_part_Slope1",
            "confirm new variable __is_part_Slope2",
            "absorb(__is_all=g##c.z __is_h=h)",
            "__is_all_d - __is_all - __is_all_Slope1 - __is_h",
            "absorb(g#c.z __so_part=h#c.w)",
            "confirm new variable __so_part",
            "absorb(__so_g=g#c.z __so_h=h#c.w)",
            "__so_all_d - __so_g_Slope1 - __so_h_Slope1",
            "e(feppml_receipt)[1,18] == 1",
            "e(feppml_receipt)[1,18] == 2",
            "e(feppml_receipt)[1,18] == 3",
            "mreldif(e(b), `is_part_b')",
            "mreldif(e(V), `so_part_V')",
            "absorb(__dupe=g##c.z) d(__dupe)",
            "absorb(__dupe2=g##c.z) xb(__dupe2_Slope1)",
            "absorb(__dupe3=g#c.z) mu(__dupe3_Slope1)",
            "absorb(__dupe4=g __dupe4=h)",
            "absorb(__chain=g##c.z __chain_Slope1=h)",
            "assert _rc == 110",
            "absorb(__occupied=g##c.z)",
            "absorb(__future=g##c.z)",
            "assert _rc == 198",
            "assert __occupied == 17",
            "assert __future_Slope1 == 23",
            "mreldif(e(b), `accepted_b') == 0",
            "END_FIXTURE|feppml_saved_slopes|rc=0",
        )
        for token in required:
            with self.subTest(token=token):
                self.assertIn(token, source)

        transaction = TRANSACTION.read_text(encoding="utf-8")
        for token in (
            "local target `\"`saved_base'_Slope`slope_index'\"'",
            "local output_saved_registry",
            "output_saved_channels=`output_saved_registry'",
            "rename `saved_shadow' `saved_target'",
            "PPML output and saved fixed-effect names must be distinct",
        ):
            self.assertIn(token, transaction)

    def test_time_series_contract_materializes_before_native_binding(self) -> None:
        source = (FIXTURES / "feppml_time_series.do").read_text(encoding="utf-8")
        for token in (
            "xtset panel period",
            "generate double __manual_lag = L.x",
            "generate double __manual_lead = F.x",
            "generate double __manual_difference = D.x",
            "missing(__manual_lead) if within_panel == 4",
            "missing(__manual_lag) if within_panel == 5",
            "y L.x F.x D.x if candidate in 2/35",
            "assert e(sample) if obs_id == 2",
            "within_panel == 7",
            "c.__manual_difference if candidate in 2/35",
            "mreldif(e(b), `ts_b')",
            "mreldif(e(V), `ts_V')",
            "END_FIXTURE|feppml_time_series|rc=0",
        ):
            self.assertIn(token, source)

    def test_time_series_fixture_covers_prediction_and_sample_boundaries(self) -> None:
        source = (FIXTURES / "feppml_time_series.do").read_text(encoding="utf-8")
        for token in (
            "predict double __ts_predict_xb if candidate in 2/35, xb",
            "assert __ts_predict_xb < . if __ts_sample",
            "assert missing(__ts_predict_xb) if !__ts_sample",
            "_b[L.x] * L.x",
            "_b[F.x] * F.x",
            "_b[D.x] * D.x",
        ):
            self.assertIn(token, source)

        transaction = TRANSACTION.read_text(encoding="utf-8")
        materialize = transaction.index("Stata, not Rust, owns time-series indexing")
        plugin_call = transaction.index("plugin call __fereg_estimate_plugin")
        self.assertLess(materialize, plugin_call)
        for token in (
            "matrix `original_model_components' = `model_components'",
            "generate double `materialized_ts' = `ts_expression'`base_source'",
            "matrix `model_components'[`component_row',9] = 3",
            "local source_has_other_use 0",
            "local source_already_reused 0",
        ):
            self.assertIn(token, transaction[materialize:plugin_call])

    def test_defaults_validation_and_closed_e_inventory_are_executable(self) -> None:
        source = (FIXTURES / "feppml_command_contract.do").read_text(encoding="utf-8")
        for token in (
            'e(vce)\'"\' == "robust"',
            'e(separation)\'"\' == "fe simplex ir"',
            'e(memory)\'"\' == "auto"',
            "e(feppml_poolsize) == 10",
            "e(feppml_requested_tolerance) == 1e-8",
            "e(feppml_iterate) == 10000",
            'e(solver)\'"\' == "auto"',
            "absorb(cl1 cl2) separation(none)",
            "solver(lsmr)",
            'e(feppml_solver_engine)\'"\', "lsmr_fused", "lsmr_joint"',
            "solver(cmg)",
            'e(feppml_solver_engine)\'"\' == "cmg_prepared"',
            'e(feppml_solver_fallback_reason)\'"\' == "cmg_numerical_nonconvergence"',
            "e(feppml_linear_route) == 2",
            "local allowed_feppml_scalars feppml_action_events_delta",
            "local actual_scalars : e(scalars)",
            "local feppml : list field in allowed_feppml_scalars",
            "assert `standard' | `feppml'",
            "local actual_macros : e(macros)",
            "local actual_matrices : e(matrices)",
            "local actual_functions : e(functions)",
            "foreach field in rss tss mss r2 r2_a rmse F df_r",
            "capture confirm matrix e(`field')",
            "memory(bogus)",
            "solver(bogus)",
            "solver(cg)",
            "fesolver(map)",
            "threads(-1)",
            "poolsize(0)",
            "tolerance(1)",
            "iterate(0)",
            "offset(off) exposure(expo)",
            "separation(fe fe)",
            "separation(none ir)",
            "separation(default mu)",
            "vce(cluster cl1 cl2 cl3 cl4 cl5 cl6)",
            "residualtype(none)",
            "eta(__bad_eta) xbd(__bad_xbd)",
            "xb(__bad_duplicate) mu(__bad_duplicate)",
            "xb(__existing_output)",
            "assert __existing_output == 0",
            "i.y c.x",
            "L.y c.x",
            "foreach operator in L F D",
            "absorb(`operator'.cl1)",
            "vce(cluster L.cl1)",
            "i.cl1#c.x",
            "c.x#c.off",
            "[aw=wt]",
            "[iw=wt]",
            "mreldif(e(V), `accepted_V') == 0",
            "END_FIXTURE|feppml_command_contract|rc=0",
        ):
            self.assertIn(token, source)

        transaction = TRANSACTION.read_text(encoding="utf-8")
        allowed_match = re.search(
            r"local allowed_feppml_scalars (?P<body>.*?)\n"
            r"local actual_scalars : e\(scalars\)",
            source,
            re.DOTALL,
        )
        self.assertIsNotNone(allowed_match)
        allowed = set(allowed_match.group("body").replace("///", " ").split())
        expected = set(
            re.findall(r"ereturn scalar (feppml_[A-Za-z0-9_]+)\s*=", transaction)
        )
        for pattern, prefix in (
            (
                r"local allocator_names (?P<body>.*?)\n\s*forvalues allocator_row",
                "feppml_",
            ),
            (
                r"local phase_names (?P<body>.*?)\n\s*forvalues phase_row",
                "feppml_time_",
            ),
            (
                r"local audit_scalar_names (?P<body>.*?)\n\s*forvalues runtime_row",
                "feppml_",
            ),
        ):
            match = re.search(pattern, transaction, re.DOTALL)
            self.assertIsNotNone(match)
            expected.update(
                prefix + name
                for name in match.group("body").replace("///", " ").split()
            )
        peaks = re.search(
            r"local peak_phase_names (?P<body>.*?)\n\s*forvalues peak_index",
            transaction,
            re.DOTALL,
        )
        self.assertIsNotNone(peaks)
        peak_names = peaks.group("body").replace("///", " ").split()
        expected.update(f"feppml_modeled_peak_{name}" for name in peak_names)
        expected.update(f"feppml_observed_peak_{name}" for name in peak_names)
        self.assertEqual(allowed, expected)

    def test_hidden_profile_closes_allocation_and_shared_surface_inventory(self) -> None:
        profiles = json.loads(self.text("ci/stata_profiles.json"))["profiles"]
        expected = {
            f"tests/stata/parity_ppmlhdfe/{hidden}"
            for hidden in SHARED.values()
        } | {
            "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_allocation.do",
            "tests/stata/documentation/estimate_ppml_hidden_help_examples.do",
        }
        hidden = profiles["ppml-hidden"]
        full = profiles["full"]
        self.assertTrue(expected.issubset(hidden["fixtures"]))
        self.assertTrue(expected.issubset(full["fixtures"]))
        self.assertTrue(hidden["run_ppml_break_fixture"])
        self.assertTrue(full["run_ppml_break_fixture"])
        self.assertTrue(expected.isdisjoint(profiles["fereg-regression"]["fixtures"]))
        activated = public_state(ROOT) == ACTIVATED
        if activated:
            self.assertIn("ppml-scientific", profiles)
        else:
            self.assertNotIn("ppml-scientific", profiles)

        allocation = (FIXTURES / "estimate_ppml_hidden_allocation.do").read_text(
            encoding="utf-8"
        )
        for token in (
            "inject(allocation)",
            "assert `allocation_rc' == 909",
            "assert `allocation_null_rc' == 909",
            "mreldif(e(b), `prior_b') == 0",
            "assert e(sample) == __allocation_prior_sample",
            "assert `\"`e(cmd)'\"' == \"\"",
            "quietly datasignature confirm",
            "capture confirm matrix e(feppml_receipt)",
            "absorb(__allocation_recovery_fe=fe) separation(none) memory(fast)",
            'assert `"`e(cmd)\'"\' == "_fereg_estimate_ppml"',
            "assert e(converged) == 1",
            "assert e(feppml_receipt)[1,33] == 1",
            "assert rowsof(e(feppml_fit)) == 27",
            "assert rowsof(e(feppml_runtime)) == 108",
            "__allocation_recovery_d - __allocation_recovery_fe",
            "__allocation_recovery_eta - __allocation_recovery_xb",
            "__allocation_recovery_resid -",
            "__allocation_recovery_drop == 0",
        ):
            self.assertIn(token, allocation)
        self.assertEqual(allocation.count("inject(allocation)"), 2)
        populated_failure = allocation.index("assert `allocation_rc' == 909")
        null_failure = allocation.index("assert `allocation_null_rc' == 909")
        recovery = allocation.index(
            "absorb(__allocation_recovery_fe=fe) separation(none) memory(fast)"
        )
        complete = allocation.index("assert e(feppml_receipt)[1,33] == 1")
        sentinel = allocation.index(
            "END_FIXTURE|estimate_ppml_hidden_allocation|rc=0"
        )
        self.assertLess(populated_failure, null_failure)
        self.assertLess(null_failure, recovery)
        self.assertLess(recovery, complete)
        self.assertLess(complete, sentinel)
        self.assertNotIn("inject(", allocation[recovery:complete])

    def test_breaknative_pty_sequence_is_exact_and_not_an_ado_sleep_alias(self) -> None:
        fixture = BREAK.read_text(encoding="utf-8")
        runner = BREAK_RUNNER.read_text(encoding="utf-8")
        plugin = PLUGIN.read_text(encoding="utf-8")
        self.assertEqual(fixture.count("inject(breaknative)"), 2)
        self.assertEqual(fixture.count("inject(breakpostpublic)"), 2)
        self.assertEqual(
            runner.split("def stata_quote", 1)[0].count('"FEPPML_NATIVE_BREAK_ARMED"'),
            2,
            "the ordered marker tuple must contain two genuine native entries",
        )
        self.assertEqual(runner.count('"FEPPML_BREAK_ARMED"'), 2)
        self.assertEqual(
            runner.split("def stata_quote", 1)[0].count(
                '"FEPPML_PREDICT_BREAK_ARMED"'
            ),
            1,
        )
        self.assertIn("for armed in armed_markers:", runner)
        self.assertIn("read_until_marker_file(", runner)
        self.assertIn('process_environment["FEREG_PPML_BREAK_MARKER_TOKEN"]', runner)
        self.assertIn('process_environment["TMPDIR"] = str(run_dir)', runner)
        self.assertIn("secrets.token_hex(16)", runner)
        self.assertIn("native_marker_path.unlink()", runner)
        self.assertIn('os.write(master, b"\\x03")', runner)
        self.assertIn(
            'const NATIVE_BREAK_MARKER: &str = "FEPPML_NATIVE_BREAK_ARMED\\n";',
            plugin,
        )
        self.assertIn("point == PpmlInterruptPoint::IrlsIteration", plugin)
        self.assertIn("spi.display(NATIVE_BREAK_MARKER)?", plugin)
        self.assertIn("create_native_break_marker(&action.break_marker_token)?", plugin)
        self.assertIn("loop {", plugin)
        self.assertIn("spi.check_interrupted()?", plugin)
        self.assertIn("assert_eq!(spi.store_calls, 0)", plugin)
        self.assertIn('"FEPPML_" "PREDICT_BREAK_ARMED"', fixture)
        self.assertIn("predict double __ppml_predict_break, mu", fixture)
        self.assertIn("assert `predict_break_rc' == 1", fixture)
        self.assertIn("confirm new variable __ppml_predict_break", fixture)
        self.assertIn('"END_" "FIXTURE|estimate_ppml_eclass_break|rc=0"', fixture)


if __name__ == "__main__":
    unittest.main()
