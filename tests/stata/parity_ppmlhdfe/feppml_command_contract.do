version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
clear all
set more off

set obs 96
generate long obs_id = _n
generate double x = mod(7 * _n, 29) / 14 - 1
generate double y = 0.7 + mod(11 * _n, 23) / 5
generate double off = 0.01 * mod(_n, 5)
generate double expo = exp(off)
generate double wt = 1 + mod(_n, 3)
forvalues cluster = 1/6 {
    generate byte cl`cluster' = 1 + mod(`cluster' * _n, 11 + `cluster')
}

* Freeze the no-option public defaults and the required result identities.
`feppml_test_command' y c.x
assert `"`e(cmd)'"' == "feppml"
assert strpos(`"`e(cmdline)'"', "feppml y c.x") == 1
assert `"`e(model)'"' == "poisson"
assert `"`e(depvar)'"' == "y"
assert `"`e(indepvars)'"' == "x"
assert `"`e(vce)'"' == "robust"
assert `"`e(vcetype)'"' == "Robust"
assert `"`e(separation)'"' == "fe simplex ir"
assert `"`e(memory)'"' == "auto"
assert `"`e(adjustment)'"' == "none"
assert `"`e(predict)'"' == "feppml_p"
assert `"`e(feppml_gate)'"' == "public_ppml_v1"
assert `"`e(feppml_memory_requested)'"' == "auto"
assert `"`e(feppml_action_protocol)'"' == "1.6"
assert `"`e(feppml_model_wire)'"' == "2.1"
assert `"`e(feppml_request_wire)'"' == "1.8"
assert `"`e(feppml_result_wire)'"' == "1.5"
assert `"`e(feppml_plugin)'"' == "0.5.0"
assert `"`e(solver)'"' == "auto"
assert `"`e(feppml_solver_requested)'"' == "auto"
assert `"`e(feppml_solver_selected)'"' == "map"
assert `"`e(feppml_solver_engine)'"' == "map"
assert `"`e(feppml_solver_effective)'"' == "map"
assert `"`e(feppml_solver_engine_effective)'"' == "map"
assert `"`e(feppml_solver_checkpoint)'"' == "map"
assert `"`e(feppml_solver_fallback_reason)'"' == "none"
assert e(feppml_solver_fallback_count) == 0
assert e(feppml_linear_route) == 2
assert rowsof(e(feppml_solver)) == 4
assert colsof(e(feppml_solver)) == 22
assert e(feppml_poolsize) == 10
assert e(feppml_requested_tolerance) == 1e-8
assert e(feppml_iterate) == 10000
assert e(feppml_timeit) == 0
assert e(poolsize) == 10
assert e(requested_threads) == 0
assert e(deterministic) == 0
assert e(drop_singletons) == 1
assert e(report_constant) == 1
assert e(N_hdfe) == 0
assert e(N_hdfe_extended) == 0
assert e(feppml_vce_requested) == 2
assert e(feppml_vce_effective) == 2
assert e(feppml_receipt)[1,33] == 1
assert e(converged) == 1
assert e(sample) == 1

* The canonical result inventory is closed: standard fields are explicitly
* enumerated, estimator diagnostics use only the feppml_ namespace, and no
* matrix or macro outside the approved cross-platform set may appear.
local standard_scalars N rank df_m df_resid df_structural df_a ///
    N_physical N_effective N_candidate N_full deviance ll ll_0 r2_p mu_0 ///
    chi2 df_chi2 sum_y ic inner_iterations step_halvings sumweights ///
    raw_sumweights weight_multiplier converged log_raw_sumweights ///
    log_weight_multiplier separated num_singletons drop_singletons ///
    report_constant N_hdfe N_hdfe_extended G_min N_clust N_clustervars ///
    N_clust1 N_clust2 N_clust3 N_clust4 N_clust5 poolsize ///
    requested_threads deterministic
local allowed_feppml_scalars feppml_action_events_delta ///
    feppml_action_events_end feppml_action_events_start ///
    feppml_allocation_audit_present feppml_allocation_cert_passed ///
    feppml_allocation_current feppml_allocation_growth ///
    feppml_allocation_model_ratio feppml_allocation_peak ///
    feppml_allocation_peak_delta feppml_allocation_previous ///
    feppml_allocation_start feppml_audit_current_end ///
    feppml_audit_current_growth feppml_audit_current_start ///
    feppml_audit_events_delta feppml_audit_events_end ///
    feppml_audit_events_start feppml_audit_iterations ///
    feppml_audit_linear_route feppml_audit_peak_end ///
    feppml_audit_peak_growth feppml_audit_peak_start ///
    feppml_audit_route_qualified feppml_audit_scratch_end ///
    feppml_audit_scratch_stable feppml_audit_scratch_start ///
    feppml_cert_tolerance feppml_cluster_df_cap feppml_coef_distribution ///
    feppml_connected_components feppml_diagnostic_records ///
    feppml_fallback_count feppml_fallback_mask feppml_final_hash_hi ///
    feppml_final_hash_lo feppml_fresh_warm_equivalent ///
    feppml_hash_digest_hi feppml_hash_digest_lo feppml_initial_hash_hi ///
    feppml_initial_hash_lo feppml_iterate feppml_iteration_allocation_free ///
    feppml_linear_capacity_stable feppml_linear_elem_cap_end ///
    feppml_linear_elem_cap_start feppml_linear_fe_bytes_end ///
    feppml_linear_fe_bytes_start feppml_linear_res_elem_end ///
    feppml_linear_res_elem_start feppml_linear_route ///
    feppml_linear_scratch_end feppml_linear_scratch_start ///
    feppml_linear_session_alloc_free feppml_memory_fast_bytes ///
    feppml_memory_low_bytes feppml_memory_requested_code ///
    feppml_memory_selected_bytes feppml_memory_selected_code ///
    feppml_model_rank feppml_modeled_peak_fe_recovery ///
    feppml_modeled_peak_feproj feppml_modeled_peak_final_solve ///
    feppml_modeled_peak_finite feppml_modeled_peak_inference ///
    feppml_modeled_peak_line_search feppml_modeled_peak_prep ///
    feppml_modeled_peak_publication feppml_modeled_peak_sep_fe ///
    feppml_modeled_peak_sep_ir feppml_modeled_peak_sep_mu ///
    feppml_modeled_peak_sep_simplex feppml_modeled_peak_weight ///
    feppml_modeled_scratch_bytes feppml_objective_scale_code ///
    feppml_observed_peak_fe_recovery feppml_observed_peak_feproj ///
    feppml_observed_peak_final_solve feppml_observed_peak_finite ///
    feppml_observed_peak_inference feppml_observed_peak_line_search ///
    feppml_observed_peak_prep feppml_observed_peak_publication ///
    feppml_observed_peak_sep_fe feppml_observed_peak_sep_ir ///
    feppml_observed_peak_sep_mu feppml_observed_peak_sep_simplex ///
    feppml_observed_peak_weight feppml_poolsize ///
    feppml_ppml_capacity_stable feppml_ppml_coef_cap_end ///
    feppml_ppml_coef_cap_start feppml_ppml_diag_cap_end ///
    feppml_ppml_diag_cap_start feppml_ppml_obs_cap_end ///
    feppml_ppml_obs_cap_start feppml_psd_adjusted ///
    feppml_psd_clip_relinf feppml_psd_clipped feppml_psd_eigen_relinf ///
    feppml_psd_final_min feppml_psd_final_rank feppml_psd_orthog_relinf ///
    feppml_psd_passed feppml_psd_raw_min feppml_psd_state ///
    feppml_rank_frozen feppml_rank_is_approximate ///
    feppml_repeated_baseline_growth feppml_requested_threads ///
    feppml_requested_tolerance feppml_scratch_bytes ///
    feppml_sep_executions feppml_sep_generations ///
    feppml_solver_fallback_count feppml_steady_median_ns ///
    feppml_strict_profile_rejections feppml_threads ///
    feppml_time_fe_projection feppml_time_fe_recovery ///
    feppml_time_finite_solve feppml_time_inference feppml_time_line_search ///
    feppml_time_preparation feppml_time_publication ///
    feppml_time_separation_fe feppml_time_separation_ir ///
    feppml_time_separation_mu feppml_time_separation_simplex ///
    feppml_time_strict_final_solve feppml_time_weight_refresh ///
    feppml_timeit feppml_trace_digest_hi feppml_trace_digest_lo ///
    feppml_two_consecutive feppml_vce_correction ///
    feppml_vce_correction_policy feppml_vce_effective ///
    feppml_vce_requested feppml_weight_code ///
    feppml_workspace_allocation_free
local actual_scalars : e(scalars)
foreach field of local actual_scalars {
    local standard : list field in standard_scalars
    local feppml : list field in allowed_feppml_scalars
    assert `standard' | `feppml'
}
foreach field in N rank df_m df_resid df_structural df_a N_physical ///
    N_effective N_candidate N_full deviance ll ll_0 r2_p mu_0 sum_y ic ///
    inner_iterations step_halvings sumweights converged separated ///
    num_singletons drop_singletons report_constant N_hdfe N_hdfe_extended ///
    poolsize requested_threads deterministic feppml_model_rank ///
    feppml_vce_requested feppml_vce_effective feppml_psd_passed ///
    feppml_connected_components feppml_poolsize ///
    feppml_requested_tolerance feppml_iterate feppml_timeit {
    confirm scalar e(`field')
}

local allowed_macros cmd cmdline model depvar indepvars vce vcetype absvars ///
    extended_absvars separation memory chi2type title title2 title3 ///
    adjustment offset exposure wtype wexp clustvar clustvar1 clustvar2 ///
    clustvar3 clustvar4 clustvar5 resid residual_type xb_var d_var eta_var ///
    mu_var drop_reason_var predict feppml_action_protocol feppml_model_wire ///
    feppml_request_wire feppml_result_wire feppml_plugin feppml_gate ///
    feppml_memory_requested feppml_memory_method solver feppml_solver_requested ///
    feppml_solver_selected feppml_solver_engine feppml_solver_effective ///
    feppml_solver_engine_effective feppml_solver_checkpoint ///
    feppml_solver_fallback_reason properties
local actual_macros : e(macros)
foreach field of local actual_macros {
    local allowed : list field in allowed_macros
    assert `allowed'
}

local allowed_matrices b V feppml_status feppml_fit feppml_dof feppml_fe ///
    feppml_cluster feppml_subset feppml_separation feppml_inference ///
    feppml_drop_counts feppml_separation_sequence feppml_certificate ///
    feppml_solver feppml_runtime feppml_memory feppml_receipt dof_table
local actual_matrices : e(matrices)
foreach field of local actual_matrices {
    local allowed : list field in allowed_matrices
    assert `allowed'
}
foreach field in b V feppml_status feppml_fit feppml_dof feppml_fe ///
    feppml_cluster feppml_subset feppml_separation feppml_inference ///
    feppml_drop_counts feppml_separation_sequence feppml_certificate ///
    feppml_solver feppml_runtime feppml_receipt {
    confirm matrix e(`field')
}
local actual_functions : e(functions)
local actual_functions : list sort actual_functions
assert `"`actual_functions'"' == "sample"

* OLS fit statistics and internal fereg result tables are forbidden rather
* than silently filled with PPML values.
foreach field in rss tss mss r2 r2_a rmse F df_r {
    capture confirm scalar e(`field')
    assert _rc == 111
}
foreach field in fereg_status fereg_fit fereg_dof fereg_fe ///
    fereg_certificate fereg_runtime fereg_receipt {
    capture confirm matrix e(`field')
    assert _rc == 111
}
assert `"`e(dof)'"' == ""

tempname accepted_b accepted_V
matrix `accepted_b' = e(b)
matrix `accepted_V' = e(V)
generate byte __accepted_sample = e(sample)

`feppml_test_command' y, absorb(cl1 cl2) separation(none) ///
    memory(fast) threads(1) deterministic solver(lsmr)
assert `"`e(feppml_solver_engine)'"' == "lsmr_fused"
assert `"`e(feppml_solver_engine_effective)'"' == "lsmr_fused"
assert e(feppml_solver_fallback_count) == 0
forvalues row = 1/4 {
    assert e(feppml_solver)[`row',3] == 2
    assert e(feppml_solver)[`row',5] == 2
}
assert e(feppml_solver)[2,8] > 0
assert e(feppml_solver)[3,8] == 0

`feppml_test_command' y c.x, absorb(cl1 cl2) separation(none) ///
    memory(fast) threads(1) deterministic solver(lsmr)
assert `"`e(solver)'"' == "lsmr"
assert `"`e(feppml_solver_selected)'"' == "lsmr"
assert `"`e(feppml_solver_engine)'"' == "lsmr_joint"
assert `"`e(feppml_solver_effective)'"' == "lsmr"
assert `"`e(feppml_solver_engine_effective)'"' == "lsmr_joint"
assert `"`e(feppml_solver_checkpoint)'"' == "lsmr"
assert `"`e(feppml_solver_fallback_reason)'"' == "none"
assert e(feppml_solver_fallback_count) == 0
forvalues row = 1/4 {
    assert e(feppml_solver)[`row',1] == 3
    assert e(feppml_solver)[`row',2] == 3
    assert e(feppml_solver)[`row',3] == 3
    assert e(feppml_solver)[`row',4] == 3
    assert e(feppml_solver)[`row',5] == e(feppml_solver)[`row',3]
    assert e(feppml_solver)[`row',6] == 3
    assert e(feppml_solver)[`row',7] == `row'
}
assert e(feppml_solver)[2,8] == 0
assert e(feppml_solver)[3,8] > 0
* Shared geometry may already be prepared; setup time can be zero.
assert !missing(e(feppml_solver)[2,9]) & e(feppml_solver)[2,9] >= 0
* The compatibility alias identifies the independently certified canonical
* categorical checkpoint; the requested joint LSMR solve remains above.
assert e(feppml_linear_route) == 2

`feppml_test_command' y c.x, absorb(cl1 cl2 cl3) separation(none) ///
    memory(fast) threads(1) deterministic solver(lsmr)
assert `"`e(feppml_solver_engine)'"' == "lsmr_joint"
assert `"`e(feppml_solver_engine_effective)'"' == "lsmr_joint"
assert e(feppml_solver_fallback_count) == 0
assert e(feppml_solver)[2,8] == 0
assert e(feppml_solver)[3,8] > 0

`feppml_test_command' y c.x, absorb(cl1 cl2) separation(none) ///
    memory(fast) threads(1) deterministic solver(cmg)
assert `"`e(solver)'"' == "cmg"
assert `"`e(feppml_solver_selected)'"' == "cmg"
assert `"`e(feppml_solver_engine)'"' == "cmg_prepared"
assert inlist(`"`e(feppml_solver_effective)'"', "cmg", "map")
if `"`e(feppml_solver_effective)'"' == "cmg" {
    assert `"`e(feppml_solver_engine_effective)'"' == "cmg_prepared"
    assert `"`e(feppml_solver_fallback_reason)'"' == "none"
    assert e(feppml_solver_fallback_count) == 0
}
else {
    assert `"`e(feppml_solver_engine_effective)'"' == "map"
    assert `"`e(feppml_solver_fallback_reason)'"' == "cmg_numerical_nonconvergence"
    assert e(feppml_solver_fallback_count) == 1
}
forvalues row = 1/4 {
    assert e(feppml_solver)[`row',1] == 7
    assert e(feppml_solver)[`row',2] == 7
    assert e(feppml_solver)[`row',3] == 4
    assert inlist(e(feppml_solver)[`row',4], 2, 7)
    assert e(feppml_solver)[`row',6] == 3
    assert e(feppml_solver)[`row',7] == `row'
}
assert e(feppml_solver)[1,11] + e(feppml_solver)[4,11] == ///
    e(feppml_solver_fallback_count)
assert inlist(e(feppml_linear_route), 2, 3)

`feppml_test_command' y c.x, solver(map)
assert `"`e(solver)'"' == "map"
assert `"`e(feppml_solver_requested)'"' == "map"
assert `"`e(feppml_solver_selected)'"' == "map"
assert `"`e(feppml_solver_engine)'"' == "map"
assert `"`e(feppml_solver_effective)'"' == "map"
assert `"`e(feppml_solver_checkpoint)'"' == "map"
assert e(feppml_solver_fallback_count) == 0
assert e(feppml_linear_route) == 2
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0

* The prepared CMG nuisance cap is inclusive: 65 levels contribute exactly
* 64 non-base nuisance columns after the two 67-level core effects are chosen.
preserve
clear
set obs 1000
generate double x = mod(7 * _n, 29) / 14 - 1
generate double y = 1 + mod(11 * _n, 7)
generate int __solver_core1 = mod(_n - 1, 67)
generate int __solver_core2 = mod(5 * (_n - 1) + floor((_n - 1) / 67), 67)
generate int __solver_nuisance = mod(_n - 1, 65)
capture noisily `feppml_test_command' y c.x, ///
    absorb(__solver_core1 __solver_core2 __solver_nuisance) ///
    separation(none) solver(cmg) memory(fast) threads(1) deterministic
assert _rc == 0
assert `"`e(feppml_solver_selected)'"' == "cmg"
tempname nuisance_cap_b nuisance_cap_V
matrix `nuisance_cap_b' = e(b)
matrix `nuisance_cap_V' = e(V)
replace __solver_nuisance = mod(_n - 1, 66)
capture noisily `feppml_test_command' y c.x, ///
    absorb(__solver_core1 __solver_core2 __solver_nuisance) ///
    separation(none) solver(cmg) memory(fast) threads(1) deterministic
assert _rc == 498
assert mreldif(e(b), `nuisance_cap_b') == 0
assert mreldif(e(V), `nuisance_cap_V') == 0
restore

* A nuisance FE nested in a core adds no projection direction. It must remain
* in the canonical model and saved output, under every absorb permutation.
preserve
clear
set obs 480
generate int __na = mod(_n - 1, 24)
generate int __nb = mod(5 * (_n - 1) + floor((_n - 1) / 24), 9)
generate int __nc = floor(__na / 3)
generate double x = sin(_n / 7) + cos(_n / 13)
generate double y = 1 + mod(11 * _n + __na + 3 * __nb, 17)
generate double off = .1 * sin(_n / 17)
generate int wt = 1 + mod(_n, 3)
forvalues k = 0/1 {
    local controls
    if `k' local controls c.x
    `feppml_test_command' y `controls' [fw=wt], absorb(__na __nb __nc) ///
        offset(off) solver(map) separation(none) threads(1) deterministic ///
        eta(__reference_eta) mu(__reference_mu)
    tempname nested_b nested_V nested_ll nested_dev nested_rank nested_dfa
    matrix `nested_b' = e(b)
    matrix `nested_V' = e(V)
    scalar `nested_ll' = e(ll)
    scalar `nested_dev' = e(deviance)
    scalar `nested_rank' = e(rank)
    scalar `nested_dfa' = e(df_a)
    foreach order in "a b c" "a c b" "b a c" "b c a" "c a b" "c b a" {
        local absorption
        foreach role of local order {
            local absorption `absorption' __saved_`role'=__n`role'
        }
        `feppml_test_command' y `controls' [fw=wt], absorb(`absorption') ///
            offset(off) solver(cmg) separation(none) memory(fast) ///
            threads(2) deterministic eta(__nested_eta) mu(__nested_mu) d(__nested_d)
        assert e(converged) == 1
        assert e(feppml_solver_fallback_count) == 0
        assert `"`e(feppml_solver_engine_effective)'"' == "cmg_prepared"
        assert e(N_hdfe) == 3
        assert e(rank) == `nested_rank'
        assert e(df_a) == `nested_dfa'
        assert mreldif(e(b), `nested_b') < 1e-8
        assert mreldif(e(V), `nested_V') < 1e-8
        assert reldif(e(ll), `nested_ll') < 1e-10
        assert reldif(e(deviance), `nested_dev') < 1e-10
        assert reldif(__nested_eta, __reference_eta) < 1e-8
        assert reldif(__nested_mu, __reference_mu) < 1e-8
        assert reldif(__saved_a + __saved_b + __saved_c, __nested_d) < 1e-8
        drop __saved_a __saved_b __saved_c __nested_eta __nested_mu __nested_d
    }
    drop __reference_eta __reference_mu
}
restore
`feppml_test_command' y c.x, solver(map)
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0

* Public defaults have a deliberately narrow validation surface.  All errors
* below occur without changing the accepted result or creating requested
* output variables.
capture noisily `feppml_test_command' y c.x, memory(bogus)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, solver(bogus)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, solver(cg)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, fesolver(map)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, finitesolver(auto)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, fepreconditioner(block)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, felsmrmode(fused)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, felsmrpolicy(auto)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, fecmgmode(prepared)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, solvertrace
assert _rc == 198
capture noisily `feppml_test_command' y c.x, solver(lsmr)
assert _rc == 498
capture noisily `feppml_test_command' y c.x, solver(cmg)
assert _rc == 498
generate int __solver_wide1 = _n
generate int __solver_wide2 = mod(5 * _n, 97)
generate int __solver_wide3 = mod(7 * _n, 101)
capture noisily `feppml_test_command' y c.x, ///
    absorb(__solver_wide1 __solver_wide2 __solver_wide3) ///
    keepsingletons separation(none) solver(cmg) memory(fast)
assert _rc == 498
drop __solver_wide1 __solver_wide2 __solver_wide3
capture noisily `feppml_test_command' y c.x, ///
    absorb(cl1 cl2) separation(none) solver(lsmr) memory(low)
assert _rc == 498
capture noisily `feppml_test_command' y c.x, ///
    absorb(cl1##c.x cl2) separation(none) solver(lsmr) memory(fast)
assert _rc == 498
capture noisily `feppml_test_command' y c.x, ///
    absorb(__solver_lsmr_fe=cl1 cl2) separation(none) ///
    solver(lsmr) memory(fast)
assert _rc == 0
confirm variable __solver_lsmr_fe
assert __solver_lsmr_fe < . if e(sample)
drop __solver_lsmr_fe
capture noisily `feppml_test_command' y c.x, ///
    absorb(__solver_fe=cl1 cl2) separation(none) ///
    solver(cmg) memory(fast)
assert _rc == 0
confirm variable __solver_fe
assert __solver_fe < . if e(sample)
drop __solver_fe
* Successful saved-FE fits replace e(); restore the accepted model before
* testing that the following rejected requests leave it unchanged.
`feppml_test_command' y c.x, solver(map)
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0
assert e(sample) == __accepted_sample
capture noisily `feppml_test_command' y c.x, threads(-1)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, poolsize(0)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, tolerance(1)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, iterate(0)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, offset(off) exposure(expo)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, separation(unknown)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, separation(fe fe)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, separation(none ir)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, separation(default mu)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, vce(iid)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, vce(ols)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, vce(cluster)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, ///
    vce(cluster cl1 cl2 cl3 cl4 cl5 cl6)
assert _rc == 198

capture noisily `feppml_test_command' y c.x, ///
    residuals(__bad_resid) residualtype(none)
assert _rc == 198
confirm new variable __bad_resid
capture noisily `feppml_test_command' y c.x, residualtype(response)
assert _rc == 198
capture noisily `feppml_test_command' y c.x, ///
    eta(__bad_eta) xbd(__bad_xbd)
assert _rc == 198
confirm new variable __bad_eta
confirm new variable __bad_xbd
capture noisily `feppml_test_command' y c.x, ///
    xb(__bad_duplicate) mu(__bad_duplicate)
assert _rc == 110
confirm new variable __bad_duplicate
generate double __existing_output = 0
capture noisily `feppml_test_command' y c.x, xb(__existing_output)
assert _rc == 110
assert __existing_output == 0
tsset obs_id
capture noisily `feppml_test_command' i.y c.x
assert _rc == 198
capture noisily `feppml_test_command' L.y c.x
assert _rc == 198
foreach operator in L F D {
    capture noisily `feppml_test_command' y c.x, ///
        absorb(`operator'.cl1) separation(none) keepsingletons
    assert _rc == 198
}
capture noisily `feppml_test_command' y c.x, vce(cluster L.cl1)
assert _rc == 198
capture noisily `feppml_test_command' y i.cl1#c.x, separation(none)
assert _rc == 498
capture noisily `feppml_test_command' y c.x#c.off, separation(none)
assert _rc == 498
capture noisily `feppml_test_command' y c.x [aw=wt]
assert _rc == 101
capture noisily `feppml_test_command' y c.x [iw=wt]
assert _rc == 101
capture noisily `feppml_test_command' y c.x, inject(panic)
assert _rc == 198

assert `"`e(cmd)'"' == "feppml"
assert e(sample) == __accepted_sample
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_command_contract|rc=0"
