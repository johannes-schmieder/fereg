version 18.0
clear all
set more off

set obs 96
generate long obs_id = _n
generate double x = mod(7 * _n, 23) / 11 - 1
generate byte fe = 1 + mod(_n - 1, 8)
generate double y = 0.5 + mod(5 * _n, 17)

quietly regress y c.x if obs_id <= 72
tempname prior_b prior_V
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
generate byte __allocation_prior_sample = e(sample)
local prior_cmd `"`e(cmd)'"'
local prior_N : display %21x e(N)

* The hidden native hook reaches the real fallible-allocation path at the FE
* recovery checkpoint.  It must surface as r(909) before any SPI result store.
capture noisily _fereg_estimate_ppml y c.x, ///
    absorb(__allocation_fe=fe) separation(none) memory(fast) ///
    residual(__allocation_resid) xb(__allocation_xb) ///
    d(__allocation_d) eta(__allocation_eta) mu(__allocation_mu) ///
    dropreason(__allocation_drop) inject(allocation)
local allocation_rc = _rc
assert `allocation_rc' == 909
foreach output in __allocation_fe __allocation_resid __allocation_xb ///
    __allocation_d __allocation_eta __allocation_mu __allocation_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == `"`prior_cmd'"'
local after_N : display %21x e(N)
assert `"`after_N'"' == `"`prior_N'"'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_V') == 0
assert e(sample) == __allocation_prior_sample
capture confirm matrix e(feppml_receipt)
assert _rc == 111

* A genuinely null estimation state is not equivalent to an empty posted
* estimate.  Allocation failure must restore null e() and the complete data.
ereturn clear
quietly datasignature set, reset
capture noisily _fereg_estimate_ppml y c.x, ///
    absorb(__allocation_null_fe=fe) separation(none) memory(fast) ///
    residual(__allocation_null_resid) xb(__allocation_null_xb) ///
    d(__allocation_null_d) eta(__allocation_null_eta) ///
    mu(__allocation_null_mu) dropreason(__allocation_null_drop) ///
    inject(allocation)
local allocation_null_rc = _rc
assert `allocation_null_rc' == 909
foreach output in __allocation_null_fe __allocation_null_resid ///
    __allocation_null_xb __allocation_null_d __allocation_null_eta ///
    __allocation_null_mu __allocation_null_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == ""
assert missing(e(N))
capture confirm matrix e(feppml_receipt)
assert _rc == 111
quietly datasignature confirm

* The injection is action-local.  A normal action in this same Stata process
* must still reach COMPLETE and publish the full result after both rollback
* paths above, including the genuinely null prior state.
quietly _fereg_estimate_ppml y c.x, ///
    absorb(__allocation_recovery_fe=fe) separation(none) memory(fast) ///
    residual(__allocation_recovery_resid) xb(__allocation_recovery_xb) ///
    d(__allocation_recovery_d) eta(__allocation_recovery_eta) ///
    mu(__allocation_recovery_mu) ///
    dropreason(__allocation_recovery_drop)
assert `"`e(cmd)'"' == "_fereg_estimate_ppml"
assert e(converged) == 1
assert e(N_physical) == _N
assert e(N_effective) == _N
assert e(sample) == 1
assert colsof(e(b)) == 2
assert rowsof(e(V)) == 2 & colsof(e(V)) == 2
assert e(feppml_receipt)[1,1] == 5354602
assert e(feppml_receipt)[1,2] == 1
assert e(feppml_receipt)[1,3] == 4
assert e(feppml_receipt)[1,4] == 2
assert e(feppml_receipt)[1,13] == 1
assert e(feppml_receipt)[1,14] == 1
assert e(feppml_receipt)[1,15] == 1
assert e(feppml_receipt)[1,20] == 16
assert e(feppml_receipt)[1,21] == 14
assert e(feppml_receipt)[1,22] == 5
assert e(feppml_receipt)[1,33] == 1
assert rowsof(e(feppml_fit)) == 27
assert rowsof(e(feppml_runtime)) == 108
foreach output in __allocation_recovery_fe __allocation_recovery_resid ///
    __allocation_recovery_xb __allocation_recovery_d ///
    __allocation_recovery_eta __allocation_recovery_mu ///
    __allocation_recovery_drop {
    confirm numeric variable `output'
}
assert abs(__allocation_recovery_d - __allocation_recovery_fe) <= 1e-10
assert abs(__allocation_recovery_eta - __allocation_recovery_xb - ///
    __allocation_recovery_d) <= 1e-10
assert reldif(__allocation_recovery_mu, ///
    exp(__allocation_recovery_eta)) <= 1e-10
assert abs(__allocation_recovery_resid - ///
    (y - __allocation_recovery_mu)) <= 1e-10
assert __allocation_recovery_mu > 0 & __allocation_recovery_mu < .
assert __allocation_recovery_drop == 0

display as result "END_FIXTURE|estimate_ppml_hidden_allocation|rc=0"
