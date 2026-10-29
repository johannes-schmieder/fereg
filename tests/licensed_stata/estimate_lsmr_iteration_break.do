version 18.0
clear all
set more off
args estimator workers
assert inlist(`"`estimator'"', "ols", "ppml")
assert inlist(real(`"`workers'"'), 1, 4)
assert c(processors_lic) >= `workers' & c(processors_mach) >= `workers'
local original_processors = c(processors)
set processors `workers'

set obs 720
generate long obs_id = _n
generate int first = 1 + mod(_n-1, 17)
generate int second = 1 + mod(floor((_n-1)/2), 13)
generate int third = 1 + mod(floor((_n-1)/3), 11)
generate double weight = 1 + mod(_n, 7)
generate double x = sin(_n*.31) + cos(first*.17)
generate double y = .7*x + sin(first*.11) + cos(second*.3) + ///
    sin(third*.23) + (mod(_n*37,101)-50)*.01
local command _fereg_estimate_ols_test
local options "aweight(weight) preconditioner(diagonal)"
local more_outputs "xbd(failed_xbd)"
local output_names "failed_xbd"
local receipt fereg_receipt
if `"`estimator'"' == "ppml" {
    replace y = 1 + mod(_n*7,13) + exp(.1*x)
    local command _fereg_estimate_ppml
    local options "fweight(weight) separation(none)"
    local more_outputs "eta(failed_eta) mu(failed_mu) dropreason(failed_drop)"
    local output_names "failed_eta failed_mu failed_drop"
    local receipt feppml_receipt
}
local controls "solver(lsmr) memory(fast) tolerance(1e-10) threads(`workers')"

quietly `command' y c.x, absorb(first second third) `options' `controls' residual(reference)
tempname reference_b reference_V
matrix `reference_b' = e(b)
matrix `reference_V' = e(V)
generate byte reference_sample = e(sample)
generate byte prior_sample = 0

* Both nonnull and null e() must roll back. Each marker is emitted only after
* an actual completed upstream LSMR iteration. The controller sends real Ctrl-C;
* the trace must also show the upstream solver returning Cancelled before join.
forvalues attempt=1/2 {
    if `attempt' == 1 {
        quietly regress y c.x if obs_id <= 100
        quietly replace prior_sample = e(sample)
    }
    else ereturn clear
    local prior_scalars : e(scalars)
    local prior_macros : e(macros)
    local prior_matrices : e(matrices)
    local prior_functions : e(functions)
    local scalar_index 0
    foreach item of local prior_scalars {
        local ++scalar_index
        local prior_scalar_`scalar_index' : display %21x e(`item')
    }
    local macro_index 0
    foreach item of local prior_macros {
        local ++macro_index
        local prior_macro_`macro_index' `"`e(`item')'"'
    }
    local matrix_index 0
    foreach item of local prior_matrices {
        local ++matrix_index
        tempname prior_matrix_`matrix_index'
        matrix `prior_matrix_`matrix_index'' = e(`item')
    }
    quietly datasignature set, reset
    capture noisily `command' y c.x, ///
        absorb(failed_first=first failed_second=second failed_third=third) ///
        `options' `controls' residual(failed_residual) xb(failed_xb) d(failed_d) ///
        `more_outputs' inject(lsmr_iteration_break)
    local break_rc = _rc
    assert `break_rc' == 1
    assert c(processors) == `workers'
    foreach output in failed_first failed_second failed_third failed_residual failed_xb failed_d `output_names' {
        confirm new variable `output'
    }
    quietly datasignature confirm
    local after_scalars : e(scalars)
    local after_macros : e(macros)
    local after_matrices : e(matrices)
    local after_functions : e(functions)
    foreach kind in scalars macros matrices functions {
        local missing : list prior_`kind' - after_`kind'
        local extra : list after_`kind' - prior_`kind'
        assert `"`missing'`extra'"' == ""
    }
    local scalar_index 0
    foreach item of local prior_scalars {
        local ++scalar_index
        local value : display %21x e(`item')
        assert `"`value'"' == `"`prior_scalar_`scalar_index''"'
    }
    local macro_index 0
    foreach item of local prior_macros {
        local ++macro_index
        assert `"`e(`item')'"' == `"`prior_macro_`macro_index''"'
    }
    local matrix_index 0
    foreach item of local prior_matrices {
        local ++matrix_index
        assert mreldif(e(`item'), `prior_matrix_`matrix_index'') == 0
    }
    if `attempt' == 1 assert e(sample) == prior_sample
    capture confirm matrix e(`receipt')
    assert _rc == 111

    * All work must have joined, and both the marker and cancellation state
    * must reset before the next successful action at this worker count.
    quietly `command' y c.x, absorb(first second third) `options' `controls' residual(reset)
    assert reset == reference
    assert e(sample) == reference_sample
    assert mreldif(e(b), `reference_b') == 0
    assert mreldif(e(V), `reference_V') == 0
    drop reset
}
set processors `original_processors'
di as result "FEREG_LSMR_" "BREAK_RESET_VERIFIED"
di as result "END_" "FIXTURE|estimate_lsmr_iteration_break|rc=0"
