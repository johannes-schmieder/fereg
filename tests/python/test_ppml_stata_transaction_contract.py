from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TRANSACTION = ROOT / "stata/_fereg_estimate_ppml.ado"
ADAPTER = ROOT / "stata/_fereg_ppml.ado"
PREDICT = ROOT / "stata/feppml_p.ado"
PUBLIC = ROOT / "stata/feppml.ado"
PLUGIN = ROOT / "crates/fereg-plugin/src/ppml_estimate.rs"
PLUGIN_RESULT = ROOT / "crates/fereg-plugin/src/ppml_result.rs"
PROTOCOL = ROOT / "crates/fereg-core/src/protocol.rs"
HIDDEN_FIXTURE = ROOT / "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_transaction.do"
BREAK_FIXTURE = ROOT / "tests/licensed_stata/estimate_ppml_eclass_break.do"
RUNTIME_VALIDATOR_FIXTURE = (
    ROOT / "tests/stata/parity_ppmlhdfe/estimate_ppml_runtime_validator.do"
)
BREAK_RUNNER = ROOT / "scripts/run_stata_break_test.py"


class PpmlStataTransactionContractTests(unittest.TestCase):
    def test_hidden_transaction_is_additive_and_receipt_guarded(self) -> None:
        text = TRANSACTION.read_text(encoding="utf-8")
        self.assertEqual(text.count('`"estimate-ppml"\''), 1)
        self.assertEqual(text.count("plugin call __fereg_estimate_plugin"), 1)
        for field in (
            "request_major=1",
            "request_minor=5",
            "result_major=1",
            "result_minor=2",
            "model_major=`model_major'",
            "model_minor=`model_minor'",
            "output_inference_matrix=`inference_shadow'",
            "output_drop_counts_matrix=`drop_counts_shadow'",
            "output_separation_sequence_matrix=`separation_sequence_shadow'",
            "output_solver_matrix=`solver_shadow'",
            "output_receipt_matrix=`receipt_shadow'",
        ):
            self.assertIn(field, text)
        self.assertIn("_estimates hold", text)
        self.assertIn("restore nullok", text)
        self.assertIn("nobreak {", text)
        self.assertIn("_estimates unhold `held_estimates', not", text)
        self.assertIn("`receipt'[1,33] != 1", text)
        self.assertIn("syntax namelist(min=16 max=16)", text)
        self.assertIn("REQUESTEDVCECODE(integer) DROPSINGLETONS(integer)", text)
        self.assertIn("`coefficients' != `capacity'", text)
        self.assertIn("tempname finite_result covariance_scale", text)
        self.assertIn("PPML covariance eigenvalue evidence exceeds its certified scale", text)
        self.assertNotIn("__fereg_ppml_finite_result", text)
        self.assertNotIn("__fereg_ppml_fit", text)

    def test_transaction_preserves_ppml_public_policy(self) -> None:
        text = TRANSACTION.read_text(encoding="utf-8")
        self.assertIn('local memory_mode "auto"', text)
        self.assertIn('local separation_wire "fe simplex ir"', text)
        self.assertIn("TOLerance(real 1e-8)", text)
        self.assertIn("ITERate(integer 10000)", text)
        self.assertIn('local vce_text "robust"', text)
        self.assertIn('`"`vce_kind\'"\' == "unadjusted"', text)
        self.assertIn('inlist(`"`vce_kind\'"\', "ols", "iid")', text)
        self.assertIn("is not defined for public PPML inference", text)
        self.assertIn("feppml supports at most five cluster dimensions", text)
        self.assertIn("adjustment_position=", text)
        self.assertIn("include_intercept=1", text)
        self.assertIn('local solver_mode = lower(strtrim(`"`solver\'"\'))', text)
        self.assertIn('"auto", "map", "lsmr", "cmg"', text)
        self.assertIn('exit 498', text)
        self.assertNotIn('local finite_solver', text)

    def test_public_solver_control_is_exact_and_private_spellings_are_retired(self) -> None:
        transaction = TRANSACTION.read_text(encoding="utf-8")
        adapter = ADAPTER.read_text(encoding="utf-8")
        for surface in (transaction, adapter):
            self.assertIn("SOLVer(string)", surface)
            self.assertIn('"auto", "map", "lsmr", "cmg"', surface)
        for option in (
            "FESOLVer(string)",
            "FEPREConditioner(string)",
            "FELSMRMode(string)",
            "FELSMRPolicy(string)",
            "FECMGMode(string)",
            "SOLVERTRACE",
        ):
            self.assertNotIn(option, transaction)
            self.assertNotIn(option, adapter)
        self.assertIn("solver=`solver_mode'", transaction)
        self.assertIn("output_solver_matrix=`solver_shadow'", transaction)
        self.assertIn("solver(`solver_mode')", adapter)
        for field in (
            "fe_solver=",
            "fe_preconditioner=",
            "fe_lsmr_mode=",
            "fe_lsmr_policy=",
            "fe_cmg_mode=",
            "solver_trace=",
            "solver_experiment=",
        ):
            self.assertNotIn(field, transaction)
        self.assertNotIn("FEREG_PPML_EXPERIMENTAL_SOLVERS", transaction)

    def test_outputs_are_private_until_final_commit(self) -> None:
        text = TRANSACTION.read_text(encoding="utf-8")
        plugin_call = text.index("plugin call __fereg_estimate_plugin")
        receipt_check = text.index("_fereg_eppml_validate", plugin_call)
        commit = text.index("nobreak {", receipt_check)
        self.assertLess(plugin_call, receipt_check)
        self.assertLess(receipt_check, commit)
        for output in ("residual", "xb", "mu", "dropreason"):
            self.assertIn(f"rename `{output}_shadow' `{output}'", text[commit:])
            self.assertIn(f"output_{output}", text[:plugin_call])
        self.assertIn("rename `d_shadow' `d'", text[commit:])
        self.assertIn("output_absorbed", text[:plugin_call])
        self.assertIn("rename `eta_shadow' `eta_target'", text[commit:])
        self.assertIn("output_eta", text[:plugin_call])
        self.assertIn("rename `saved_shadow' `saved_target'", text[commit:])
        self.assertIn("output_saved_count=`output_saved_count'", text[:plugin_call])
        self.assertIn("saved(`requested_saved_count')", text[receipt_check:commit])

    def test_hidden_ado_and_native_action_use_the_same_closed_key_set(self) -> None:
        ado = TRANSACTION.read_text(encoding="utf-8")
        rust = PLUGIN.read_text(encoding="utf-8")
        request = re.search(
            r"local request_arguments `\"(?P<body>.*?)\"'\n",
            ado,
            re.DOTALL,
        )
        self.assertIsNotNone(request)
        ado_keys = {
            match.group(1)
            for match in re.finditer(r"(?:^|;)([a-z_]+)=", request.group("body"))
        }
        if "inject=`plugin_inject';" in ado:
            ado_keys.add("inject")
        if "break_marker_token=`break_marker_token';" in ado:
            ado_keys.add("break_marker_token")
        allowed = re.search(
            r"const ALLOWED: &\[&str\] = &\[(?P<body>.*?)\n\s*\];",
            rust,
            re.DOTALL,
        )
        self.assertIsNotNone(allowed)
        rust_keys = set(re.findall(r'"([a-z_]+)"', allowed.group("body")))
        self.assertEqual(ado_keys, rust_keys)

    def test_protocol_uses_ppml_request_15_and_result_12(self) -> None:
        protocol = PROTOCOL.read_text(encoding="utf-8")
        for token in (
            "PPML_REQUEST_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 5)",
            "PPML_RESULT_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 2)",
            "PPML_RESULT_INFERENCE_ROWS: usize = 16",
            "PPML_RESULT_DROP_COUNT_ROWS: usize = 14",
            "PPML_RESULT_SEPARATION_SEQUENCE_ROWS: usize = 5",
            "PPML_RESULT_RECEIPT_COLUMNS: usize = 33",
        ):
            self.assertIn(token, protocol)

    def test_result_12_evidence_layout_and_publication_are_exact(self) -> None:
        text = TRANSACTION.read_text(encoding="utf-8")

        for token in (
            "matrix `inference_shadow' = J(16, 1, 0)",
            "matrix `drop_counts_shadow' = J(14, 2, 0)",
            "matrix `separation_sequence_shadow' = J(5, 13, 0)",
            "matrix `solver_shadow' = J(4, 22, .)",
            "matrix `receipt_shadow' = J(1, 33, 0)",
            "rowsof(`inference') != 16 | colsof(`inference') != 1",
            "rowsof(`drop_counts') != 14 | colsof(`drop_counts') != 2",
            "rowsof(`separation_sequence') != 5 | colsof(`separation_sequence') != 13",
            "rowsof(`solver') != 4 | colsof(`solver') != 22",
            "rowsof(`receipt') != 1 | colsof(`receipt') != 33",
            "`receipt'[1,3] != 2",
            "`receipt'[1,33] != 1",
        ):
            self.assertIn(token, text)

        def stripe(start: str, end: str) -> list[str]:
            match = re.search(
                re.escape(start) + r" (?P<body>.*?)\n\s*" + re.escape(end),
                text,
                re.DOTALL,
            )
            self.assertIsNotNone(match)
            return match.group("body").replace("///", " ").split()

        self.assertEqual(
            stripe(
                "matrix rownames `inference_shadow' =",
                "matrix colnames `inference_shadow'",
            ),
            [
                "requested_vce_code",
                "effective_vce_code",
                "correction",
                "correction_policy_code",
                "coefficient_distribution_code",
                "psd_state",
                "psd_adjusted",
                "psd_clipped_eigenvalues",
                "psd_raw_minimum_eigenvalue",
                "psd_final_minimum_eigenvalue",
                "psd_final_rank",
                "psd_eigensystem_recon_relinf",
                "psd_orthogonality_relinf",
                "psd_clipped_recon_relinf",
                "certificate_tolerance",
                "psd_passed",
            ],
        )
        self.assertEqual(
            stripe(
                "matrix rownames `drop_counts_shadow' =",
                "matrix colnames `drop_counts_shadow'",
            ),
            [
                "retained",
                "excluded_if_in",
                "missing_outcome",
                "missing_regressor",
                "invalid_or_missing_weight",
                "missing_fixed_effect",
                "missing_cluster",
                "missing_adjustment",
                "singleton",
                "fixed_effect_separation",
                "simplex_separation",
                "ir_separation",
                "mu_separation",
                "other_model_exclusion",
            ],
        )
        self.assertEqual(
            stripe(
                "matrix colnames `separation_sequence_shadow' =",
                "matrix rownames `certificate_shadow'",
            ),
            [
                "method",
                "requested",
                "registered_order",
                "executions",
                "first_trace_row",
                "last_trace_row",
                "status_mask",
                "total_iterations",
                "direct_drops",
                "singleton_drops",
                "first_generation",
                "last_generation",
                "all_certificates_passed",
            ],
        )
        self.assertIn(
            "matrix rownames `separation_sequence_shadow' = singleton fe simplex ir mu",
            text,
        )
        self.assertEqual(
            stripe(
                "matrix colnames `solver_shadow' =",
                "matrix rownames `runtime_shadow'",
            ),
            [
                "requested_family",
                "selected_family",
                "selected_engine",
                "effective_family",
                "effective_engine",
                "checkpoint_family",
                "row_engine",
                "ordinary_frames",
                "checkpoint_frames",
                "retries",
                "fallback_count",
                "fallback_reason",
                "iterations",
                "setup_seconds",
                "solve_seconds",
                "certification_seconds",
                "absinf_max",
                "relinf_max",
                "rel2_max",
                "normal_equation_max",
                "projection_absinf_max",
                "backward_error_max",
            ],
        )
        self.assertIn(
            "matrix rownames `solver_shadow' = map lsmr_fused lsmr_joint cmg_prepared",
            text,
        )
        self.assertEqual(
            stripe(
                "matrix colnames `receipt_shadow' =",
                "matrix rownames `receipt_shadow'",
            ),
            [
                "magic",
                "major",
                "minor",
                "vce",
                "coefficients",
                "covariance_dimension",
                "N_original",
                "N_candidate",
                "N_final",
                "cluster_dimensions",
                "fixed_effect_rows",
                "subset_rows",
                "sample_present",
                "residual_present",
                "certificates_passed",
                "separated",
                "separation_rows",
                "saved_channels",
                "adjustment",
                "inference_rows",
                "drop_count_rows",
                "sequence_rows",
                "separation_executions",
                "separation_generations",
                "initial_hash_hi",
                "initial_hash_lo",
                "final_hash_hi",
                "final_hash_lo",
                "trace_digest_hi",
                "trace_digest_lo",
                "hash_digest_hi",
                "hash_digest_lo",
                "complete",
            ],
        )

        for matrix_name in (
            "feppml_inference",
            "feppml_drop_counts",
            "feppml_separation_sequence",
            "feppml_solver",
            "feppml_receipt",
        ):
            self.assertIn(f"ereturn matrix {matrix_name} =", text)
        for scalar_name in (
            "feppml_vce_requested",
            "feppml_vce_effective",
            "feppml_vce_correction",
            "feppml_vce_correction_policy",
            "feppml_coef_distribution",
            "feppml_psd_state",
            "feppml_psd_adjusted",
            "feppml_psd_clipped",
            "feppml_psd_raw_min",
            "feppml_psd_final_min",
            "feppml_psd_final_rank",
            "feppml_psd_eigen_relinf",
            "feppml_psd_orthog_relinf",
            "feppml_psd_clip_relinf",
            "feppml_cert_tolerance",
            "feppml_psd_passed",
            "feppml_sep_executions",
            "feppml_sep_generations",
            "feppml_solver_fallback_count",
            "feppml_initial_hash_hi",
            "feppml_initial_hash_lo",
            "feppml_final_hash_hi",
            "feppml_final_hash_lo",
            "feppml_trace_digest_hi",
            "feppml_trace_digest_lo",
            "feppml_hash_digest_hi",
            "feppml_hash_digest_lo",
        ):
            self.assertIn(f"ereturn scalar {scalar_name} =", text)

    def test_solver_fallback_reason_is_engine_row_local(self) -> None:
        native = PLUGIN_RESULT.read_text(encoding="utf-8")
        plugin = PLUGIN.read_text(encoding="utf-8")
        stata = TRANSACTION.read_text(encoding="utf-8")
        self.assertIn("solver_row_fallback_reason(", native)
        self.assertIn("engine.fallback_count,", native)
        self.assertIn("let cmg_start = 3 * SOLVER_COLUMNS;", plugin)
        self.assertIn(
            "(row_fallback_count == 0) != (row_fallback_reason == 0.0)",
            plugin,
        )
        self.assertIn(
            "(`solver'[`row',11] == 0) != (`solver'[`row',12] == 0)",
            stata,
        )
        self.assertIn("local failed_attempt", stata)
        self.assertIn("!`failed_attempt'", stata)
        self.assertIn("(`solver'[1,12] == 1) | `solver'[4,12] == 2", stata)
        self.assertIn("(`solver'[1,12] == 3 & `solver'[4,12] != 3)", stata)

    def test_public_command_is_the_one_call_wrapper(self) -> None:
        public = PUBLIC.read_text(encoding="utf-8")
        self.assertIn("_fereg_ppml `0'", public)
        self.assertNotIn("exit 498", public)
        self.assertNotIn("_fereg_estimate_ppml", public)

    def test_staged_public_adapter_has_the_closed_ppml_policy(self) -> None:
        adapter = ADAPTER.read_text(encoding="utf-8")
        self.assertEqual(adapter.count("_fereg_estimate_ppml `varlist'"), 1)
        self.assertIn('inlist(`"`weight\'"\', "aweight", "iweight")', adapter)
        self.assertIn("exit 101", adapter)
        self.assertIn("TOLerance(real 1e-8)", adapter)
        self.assertIn("ITERate(integer 10000)", adapter)
        self.assertIn("MEMory(string) POOLSize(integer 10) THREADS(integer 0)", adapter)
        self.assertIn("SOLVer(string)", adapter)
        for private_option in (
            "SAVEFEs",
            "FINITESOLVer",
            "VERBose",
            "SEPTOLerance",
            "SEPITERate",
            "SIMPLEXITERate",
            "SIMPLEXCELLS",
        ):
            self.assertNotIn(private_option, adapter)
        self.assertIn("solver(`solver_mode')", adapter)
        self.assertNotIn("finitesolver", adapter.lower())
        self.assertIn("publicwexp", adapter)
        self.assertIn("generate double `public_weight'", adapter)
        self.assertIn("local public_display = c(noisily)", adapter)
        self.assertIn('local display_option "publicdisplay"', adapter)
        self.assertIn("publicmode publiccmdline", adapter)
        self.assertNotIn("ereturn ", adapter)
        self.assertNotIn("di as txt", adapter)
        self.assertNotIn("INJECT(string)", adapter)
        transaction = TRANSACTION.read_text(encoding="utf-8")
        for token in (
            'ereturn local cmd "feppml"',
            'ereturn local predict "feppml_p"',
            'ereturn local feppml_gate "public_ppml_v1"',
            'ereturn scalar feppml_timeit',
            'if `"`inject\'"\' == "postpublic" exit 3498',
            'if `"`publictimeit\'"\' != "" {',
            'if `"`publicdisplay\'"\' != "" ereturn display',
        ):
            self.assertIn(token, transaction)
        display = transaction.index('if `"`publicdisplay\'"\' != "" ereturn display')
        public_break = transaction.index('if `"`inject\'"\' == "breakpostpublic"')
        commit = transaction.index("nobreak {", public_break)
        self.assertLess(public_break, commit)
        unhold = transaction.index("_estimates unhold `held_estimates', not", commit)
        self.assertLess(commit, unhold)
        self.assertLess(unhold, display)
        self.assertNotIn("feppml_verbose", transaction)

    def test_prediction_surface_matches_adr_0016(self) -> None:
        predict = PREDICT.read_text(encoding="utf-8")
        for option in (
            "MU",
            "XB",
            "D",
            "ABSorbed",
            "ETA",
            "XBD",
            "RESponse",
            "SCORE",
            "PEARson",
            "WORKing",
            "DEViance",
            "ANScombe",
        ):
            self.assertIn(option, predict)
        self.assertIn("2 * `mean_work'", predict)
        self.assertIn("1.5 *", predict)
        self.assertIn("abs(`relative_work') <= 1e-4", predict)
        self.assertIn("PPML deviance is undefined for a negative outcome", predict)
        self.assertNotIn("plugin call", predict)

        fixture = (ROOT / "tests/stata/parity_ppmlhdfe/feppml_scientific_contract.do").read_text(
            encoding="utf-8"
        )
        for token in (
            "replace y = -1 in 1",
            "assert _rc == 430",
            "confirm new variable __negative_deviance",
            "drop y",
            "assert _rc == 111",
            "confirm new variable __deleted_y_deviance",
            "drop __retained_d",
            "confirm new variable __deleted_d_prediction",
            'generate str8 string_fe = "fe"',
            "vce(cluster string_cluster)",
            "generate strL long_fe = string_fe",
            "generate strL long_cluster = string_cluster",
            "estimates store __ppml_named",
            "assert __sort_position == _n",
            "estimates restore __ppml_named",
        ):
            self.assertIn(token, fixture)

    def test_native_tables_are_bound_back_to_parser_owned_metadata(self) -> None:
        text = TRANSACTION.read_text(encoding="utf-8")
        for token in (
            "fixed-effect intercept identity disagrees with absorb()",
            "fixed-effect slope identity disagrees with absorb()",
            "cluster arity disagrees with vce(cluster ...)",
            "coefficient statuses disagree with finite rank",
            "omitted covariance row was not exactly zero-filled",
        ):
            self.assertIn(token, text)
        self.assertIn("if `fit_shadow'[11,1] < . {", text)
        self.assertEqual(text.count("ereturn scalar chi2 ="), 1)
        self.assertEqual(text.count("ereturn scalar df_chi2 ="), 1)
        self.assertNotIn("fvexpand `indepvars' if `sample_shadow'", text)
        self.assertIn('local posted_names `"`coefficient_names\' _cons"\'', text)

    def test_runtime_row_stripe_is_the_exact_frozen_108_row_layout(self) -> None:
        text = TRANSACTION.read_text(encoding="utf-8")
        native = PLUGIN_RESULT.read_text(encoding="utf-8")
        self.assertIn("const RUNTIME_ROWS: usize = 108;", native)
        self.assertIn("matrix `runtime_shadow' = J(108, 1, .)", text)
        self.assertIn("rowsof(`runtime') != 108", text)
        match = re.search(
            r"matrix rownames `runtime_shadow' = (?P<body>.*?)\n\s*"
            r"matrix colnames `runtime_shadow'",
            text,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        names = match.group("body").replace("///", " ").split()
        self.assertEqual(
            names,
            [
                "total_seconds",
                "outer_iterations",
                "inner_iterations",
                "step_halvings",
                "memory_requested",
                "memory_selected",
                "linear_route",
                "fallback_mask",
                "fallback_count",
                "threads_requested",
                "threads_effective",
                "modeled_fast_bytes",
                "modeled_low_bytes",
                "modeled_selected_bytes",
                "modeled_scratch_bytes",
                "observed_scratch_bytes",
                "allocation_previous",
                "allocation_start",
                "allocation_current",
                "allocation_peak",
                "allocation_peak_delta",
                "allocation_growth",
                "allocation_model_ratio",
                "workspace_allocation_free",
                "iteration_allocation_free",
                "diagnostic_records",
                "strict_profile_rejections",
                "time_preparation",
                "time_separation_fe",
                "time_separation_simplex",
                "time_separation_ir",
                "time_separation_mu",
                "time_weight_response_refresh",
                "time_fe_projection",
                "time_finite_solve",
                "time_objective_line_search",
                "time_strict_final_solve",
                "time_inference",
                "time_fe_recovery",
                "time_publication",
                "allocation_audit_present",
                "audit_iterations",
                "audit_events_start",
                "audit_events_end",
                "audit_events_delta",
                "audit_current_start",
                "audit_current_end",
                "audit_current_growth",
                "audit_peak_start",
                "audit_peak_end",
                "audit_peak_growth",
                "ppml_obs_capacity_start",
                "ppml_coef_capacity_start",
                "ppml_diag_capacity_start",
                "ppml_obs_capacity_end",
                "ppml_coef_capacity_end",
                "ppml_diag_capacity_end",
                "ppml_capacity_stable",
                "linear_element_capacity_start",
                "linear_resident_elements_start",
                "linear_scratch_bytes_start",
                "linear_fe_topology_bytes_start",
                "linear_element_capacity_end",
                "linear_resident_elements_end",
                "linear_scratch_bytes_end",
                "linear_fe_topology_bytes_end",
                "linear_capacity_stable",
                "audit_scratch_bytes_start",
                "audit_scratch_bytes_end",
                "audit_scratch_stable",
                "audit_linear_route",
                "audit_route_qualified",
                "linear_session_allocation_free",
                "fresh_warm_equivalent",
                "steady_iteration_median_ns",
                "allocation_certificate_passed",
                "two_consecutive_convergence",
                "rank_frozen",
                "repeated_baseline_growth",
                "action_events_start",
                "action_events_end",
                "action_events_delta",
                "modeled_peak_prep",
                "modeled_peak_sep_fe",
                "modeled_peak_sep_simplex",
                "modeled_peak_sep_ir",
                "modeled_peak_sep_mu",
                "modeled_peak_weight_refresh",
                "modeled_peak_fe_projection",
                "modeled_peak_finite_solve",
                "modeled_peak_line_search",
                "modeled_peak_final_solve",
                "modeled_peak_inference",
                "modeled_peak_fe_recovery",
                "modeled_peak_publication",
                "observed_peak_prep",
                "observed_peak_sep_fe",
                "observed_peak_sep_simplex",
                "observed_peak_sep_ir",
                "observed_peak_sep_mu",
                "observed_peak_weight_refresh",
                "observed_peak_fe_projection",
                "observed_peak_finite_solve",
                "observed_peak_line_search",
                "observed_peak_final_solve",
                "observed_peak_inference",
                "observed_peak_fe_recovery",
                "observed_peak_publication",
            ],
        )
        self.assertEqual(len(names), 108)
        self.assertTrue(all(len(name) <= 32 for name in names))
        for token in (
            "runtime[40] = 1.0",
            "runtime[76] = f64::from(convergence.two_consecutive_convergence)",
            "runtime[77] = f64::from(convergence.rank_frozen)",
            "runtime[78] = telemetry_i128(",
            "runtime[79] = telemetry_u128(",
            "runtime[80] = telemetry_u128(",
            "runtime[81] = telemetry_u128(",
            "runtime[82 + index] = telemetry_u128(",
            "runtime[95 + index] =",
            "runtime_optional.extend(16..=22)",
            "runtime_optional.extend(27..40)",
            "runtime_optional.extend(41..76)",
            "runtime_optional.push(78)",
            "runtime_optional.extend(79..82)",
            "runtime_optional.extend(95..108)",
        ):
            self.assertIn(token, native)

        scalar_match = re.search(
            r"local audit_scalar_names (?P<body>.*?)\n\s*"
            r"forvalues runtime_row = 41/82",
            text,
            re.DOTALL,
        )
        self.assertIsNotNone(scalar_match)
        scalar_names = scalar_match.group("body").replace("///", " ").split()
        self.assertEqual(len(scalar_names), 42)
        self.assertTrue(all(len(f"feppml_{name}") <= 32 for name in scalar_names))
        phase_scalar_match = re.search(
            r"local phase_names (?P<body>.*?)\n\s*"
            r"forvalues phase_row = 28/40",
            text,
            re.DOTALL,
        )
        self.assertIsNotNone(phase_scalar_match)
        phase_scalar_names = (
            phase_scalar_match.group("body").replace("///", " ").split()
        )
        self.assertEqual(len(phase_scalar_names), 13)
        self.assertTrue(
            all(len(f"feppml_time_{name}") <= 32 for name in phase_scalar_names)
        )
        literal_scalar_names = re.findall(
            r"ereturn scalar ([A-Za-z_][A-Za-z0-9_]*)\s*=", text
        )
        self.assertTrue(literal_scalar_names)
        self.assertTrue(
            all(len(name) <= 32 for name in literal_scalar_names),
            [name for name in literal_scalar_names if len(name) > 32],
        )
        for prefix in ("feppml_modeled_peak_", "feppml_observed_peak_"):
            self.assertTrue(
                all(
                    len(prefix + suffix) <= 32
                    for suffix in (
                        "prep",
                        "sep_fe",
                        "sep_simplex",
                        "sep_ir",
                        "sep_mu",
                        "weight",
                        "feproj",
                        "finite",
                        "line_search",
                        "final_solve",
                        "inference",
                        "fe_recovery",
                        "publication",
                    )
                )
            )

    def test_fit_row_stripe_is_the_exact_frozen_27_row_layout(self) -> None:
        text = TRANSACTION.read_text(encoding="utf-8")
        match = re.search(
            r"matrix rownames `fit_shadow' = (?P<body>.*?)\n\s*"
            r"matrix colnames `fit_shadow'",
            text,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        names = match.group("body").replace("///", " ").split()
        self.assertEqual(
            names,
            [
                "N_physical",
                "rank_finite",
                "rank_absorbed",
                "df_structural",
                "df_reported",
                "deviance",
                "ll",
                "ll_0",
                "r2_p",
                "mu_0",
                "chi2",
                "df_chi2",
                "sum_y",
                "outer_iterations",
                "inner_iterations",
                "step_halvings",
                "N_effective",
                "sumweights",
                "raw_sumweights",
                "weight_multiplier",
                "weight_code",
                "N_candidate",
                "N_original",
                "converged",
                "log_raw_sumweights",
                "log_weight_multiplier",
                "objective_scale_code",
            ],
        )
        for token in (
            "matrix `fit_shadow' = J(27, 1, .)",
            "rowsof(`fit') != 27",
            "if `fit_shadow'[20,1] < . ereturn scalar weight_multiplier",
            "if `fit_shadow'[25,1] < . ereturn scalar log_raw_sumweights",
            "ereturn scalar log_weight_multiplier = `fit_shadow'[26,1]",
            "ereturn scalar feppml_objective_scale_code = `fit_shadow'[27,1]",
        ):
            self.assertIn(token, text)

    def test_every_transaction_failure_path_covers_saved_fe_rollback(self) -> None:
        hidden = HIDDEN_FIXTURE.read_text(encoding="utf-8")
        for stage in (
            "posthold",
            "postplugin",
            "poststripe",
            "postereturn",
            "postscalars",
            "postdiagnostics",
            "precommit",
        ):
            self.assertIn(stage, hidden)
        for token in (
            "foreach unsupported_vce in iid ols",
            "assert _rc == 198",
            "confirm new variable `output_prefix'_`suffix'",
            "inject(postpublic)",
            "absorb(__public_tail_fe=fe)",
            "_fereg_ppml y c.x, inject(panic)",
            '"0b.group 1o.group 2.group _cons"',
            "feppml_p float __ppml_overflow",
            "assert _rc == 430",
            "confirm new variable __ppml_overflow",
            'e(feppml_gate)\'"\' == "public_ppml_v1"',
            "e(feppml_status)[2,2] == 3",
            "absorb(__failed_fe=fe)",
            "confirm new variable __failed_fe",
            "absorb(__panic_fe=fe)",
            "confirm new variable __panic_fe",
            "foreach mutation in mutatewire mutatesemantic mutatesolver mutateoutput",
            'cond(`"`mutation\'"\' == "mutatewire", 499, 3499)',
            "absorb(__class_fe=fe)",
            "confirm new variable __class_fe",
            "absorb(__null_fe=fe)",
            "confirm new variable __null_fe",
            "absorb(__null_public_fe=fe)",
            'publiccmdline("feppml guarded null tail")',
            "assert missing(e(N))",
            "quietly datasignature confirm",
            "capture confirm scalar e(chi2)",
            "capture confirm scalar e(df_chi2)",
        ):
            self.assertIn(token, hidden)
        self.assertNotIn("unsupported_solver", hidden)
        self.assertNotIn("feppml_verbose", hidden)
        self.assertEqual(hidden.count("foreach stage in posthold postplugin poststripe"), 2)

        transaction = TRANSACTION.read_text(encoding="utf-8")
        for token in (
            '"breakpostplugin", "breakpostpublic", "mutatewire"',
            '"mutatesemantic", "mutatesolver", "mutateoutput", "panic"',
            'if `"`inject\'"\' == "mutatewire" matrix `receipt_shadow\'[1,1] = 0',
            'if `"`inject\'"\' == "mutatesemantic" matrix `certificate_shadow\'[16,1] = 0',
            'if `"`inject\'"\' == "mutatesolver" matrix `solver_shadow\'[1,1] = 0',
            'if `"`inject\'"\' == "mutateoutput" replace `sample_shadow\' = 2 in 1',
            "if `validation_rc' == 499 exit 499",
            "exit 3499",
        ):
            self.assertIn(token, transaction)

        broken = BREAK_FIXTURE.read_text(encoding="utf-8")
        for token in (
            "absorb(__ppml_native_fe=fe)",
            "absorb(__ppml_break_fe=fe)",
            "absorb(__ppml_native_null_fe=fe)",
            "absorb(__ppml_null_fe=fe)",
            "inject(breaknative)",
            "inject(breakpostpublic)",
            "publictimeit publicdisplay",
            "assert `native_break_rc' == 1",
            "assert `break_rc' == 1",
            "assert `native_null_break_rc' == 1",
            "assert `null_break_rc' == 1",
            "assert mreldif(e(b), `prior_b') == 0",
            "assert mreldif(e(V), `prior_V') == 0",
            "assert e(sample) == __ppml_prior_sample",
            "assert missing(e(N))",
            "quietly datasignature confirm",
            "publiccmdline(\"feppml licensed Break prior\")",
            "publiccmdline(\"feppml licensed Break null\")",
        ):
            self.assertIn(token, broken)
        self.assertEqual(broken.count("inject(breaknative)"), 2)
        self.assertEqual(broken.count("inject(breakpostpublic)"), 2)
        runner = BREAK_RUNNER.read_text(encoding="utf-8")
        self.assertIn('"FEPPML_NATIVE_BREAK_ARMED"', runner)
        self.assertIn('"FEPPML_BREAK_ARMED"', runner)
        self.assertIn('for armed in armed_markers:', runner)
        self.assertIn("read_until_marker_file(", runner)
        self.assertIn("marker_path.read_bytes()", runner)
        self.assertIn("native_marker_path.unlink()", runner)
        self.assertIn("secrets.token_hex(16)", runner)
        self.assertIn('process_environment["TMPDIR"] = str(run_dir)', runner)
        self.assertIn('os.write(master, b"\\x03")', runner)

    def test_predict_validates_every_private_output_atomically(self) -> None:
        text = PREDICT.read_text(encoding="utf-8")
        self.assertIn("tempvar prediction_work", text)
        predict_main = text.split(
            "program define _fereg_ppml_prediction_validate", 1
        )[0]
        self.assertEqual(
            [
                line.strip()
                for line in predict_main.splitlines()
                if "`varlist'" in line
            ],
            ["nobreak rename `prediction_work' `varlist'"] * 9,
        )
        self.assertNotRegex(
            text,
            r"(?m)^\s*(?:quietly\s+)?(?:generate|replace|_predict)\b[^\n]*`varlist'",
        )
        validations = [
            match.start()
            for match in re.finditer(
                r"_fereg_ppml_prediction_validate `prediction_work'", text
            )
        ]
        commits = [
            match.start()
            for match in re.finditer(
                r"(?m)^\s*nobreak rename `prediction_work' `varlist'\s*$", text
            )
        ]
        self.assertEqual(len(validations), 9)
        self.assertEqual(len(commits), len(validations))
        for index, (validation, commit) in enumerate(zip(validations, commits)):
            self.assertLess(validation, commit)
            if index + 1 < len(validations):
                self.assertLess(commit, validations[index + 1])

        for token in (
            "program define _fereg_ppml_prediction_validate",
            "d and absorbed are aliases and may not be combined",
            "eta and xbd are aliases and may not be combined",
            "if e(N_hdfe) == 0",
            "requires d(), eta(), or mu() at estimation time",
            "capture assert `validity' if `touse' & e(sample)",
            "local domain_rc = _rc",
            "if `domain_rc' == 1 exit 1",
            "local validation_rc = _rc",
            "drop `varlist'",
            "if `validation_rc' == 1 exit 1",
            "exit 430",
            "touse(`touse') positive",
            "touse(`touse') nonnegative",
        ):
            self.assertIn(token, text)

        # Licensed non-Break prediction failure reaches validation after the
        # private value is constructed and still leaves no requested target.
        hidden_transaction = HIDDEN_FIXTURE.read_text(encoding="utf-8")
        overflow = hidden_transaction.index(
            "feppml_p float __ppml_overflow if _n == 1, mu"
        )
        overflow_rc = hidden_transaction.index("assert _rc == 430", overflow)
        overflow_absent = hidden_transaction.index(
            "confirm new variable __ppml_overflow", overflow_rc
        )
        self.assertLess(overflow, overflow_rc)
        self.assertLess(overflow_rc, overflow_absent)

    def test_runtime_validator_covers_result_12_and_runtime_failures(self) -> None:
        fixture = RUNTIME_VALIDATOR_FIXTURE.read_text(encoding="utf-8")
        for token in (
            "matrix `inference' = J(16,1,0)",
            "matrix `drop_counts' = J(14,2,0)",
            "matrix `separation_sequence' = J(5,13,0)",
            "matrix `solver' = J(4,22,.)",
            "CMG fallback reason/count belong only to the CMG engine row",
            "matrix `bad'[4,11] = 1",
            "matrix `bad'[4,12] = 1",
            "matrix `bad'[1,12] = 1",
            "requestedsolvercode(7)",
            "matrix `runtime' = J(108,1,.)",
            "matrix `receipt' = J(1,33,0)",
            "matrix `receipt'[1,3] = 2",
            "matrix `receipt'[1,33] = 1",
            "requestedvcecode(2)",
            "dropsingletons(1)",
            "requestedsolvercode(1)",
            "matrix `bad' = J(15,1,0)",
            "matrix `bad' = J(13,2,0)",
            "matrix `bad' = J(5,12,0)",
            "matrix `bad' = J(1,32,0)",
            "matrix `bad' = J(4,21,.)",
            "foreach wiremutation in magic major minor inference_rows drop_rows sequence_rows complete",
            "foreach mutation in requested correction pass",
            "foreach mutation in reason retained negative",
            "foreach mutation in method requested execution certificate",
            "foreach mutation in executions generation_bound execution_bound fractional_hash oversized_hash",
            "The fixed sequence and bounded aggregate are two views of the same ledger",
            "matrix `runtime'[67,1] = 0",
            "foreach mutation in route qualified certificate scratch modeled auditpartial",
            "matrix `bad'[79,1] = 19",
            "matrix `bad'[80,1] = 10",
            "capacity(3)",
            "assert _rc == 499",
            "assert _rc == 3499",
            "END_FIXTURE|estimate_ppml_runtime_validator|rc=0",
        ):
            self.assertIn(token, fixture)


if __name__ == "__main__":
    unittest.main()
