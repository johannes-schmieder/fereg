version 18.0
clear all
set more off

local original_processors = c(processors)
local host_processors = min(4, c(processors_lic), c(processors_mach))
set processors `host_processors'

set obs 80
generate long obs_id = _n
generate double x = mod(_n, 11) / 5 - 1
generate byte fe = 1 + mod(_n - 1, 8)
generate double y = 0.5 + mod(7 * _n, 13)

quietly regress y c.x if obs_id <= 60
tempname prior_b prior_V
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
generate byte __ppml_prior_sample = e(sample)
local prior_cmd `"`e(cmd)'"'
local prior_N : display %21x e(N)

* First interrupt from inside the first PPML IRLS checkpoint.  The plugin emits
* the PTY marker only after entering the core on Stata's caller thread, then
* polls there until the controller sends Ctrl-C.  No native store, result
* stripe, e() metadata, or requested variable may escape this boundary.
capture noisily _fereg_estimate_ppml y c.x, ///
    absorb(__ppml_native_fe=fe) separation(none) memory(fast) ///
    residual(__ppml_native_resid) xb(__ppml_native_xb) ///
    d(__ppml_native_d) eta(__ppml_native_eta) mu(__ppml_native_mu) ///
    dropreason(__ppml_native_drop) threads(`host_processors') inject(breaknative)
local native_break_rc = _rc
assert c(processors) == `host_processors'
assert `native_break_rc' == 1
foreach output in __ppml_native_fe __ppml_native_resid __ppml_native_xb ///
    __ppml_native_d __ppml_native_eta __ppml_native_mu __ppml_native_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == `"`prior_cmd'"'
local native_after_N : display %21x e(N)
assert `"`native_after_N'"' == `"`prior_N'"'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_V') == 0
assert e(sample) == __ppml_prior_sample
capture confirm matrix e(feppml_receipt)
assert _rc == 111

* Then interrupt after all public metadata and timeit output have been posted,
* but before the guarded transaction commits.
capture noisily _fereg_estimate_ppml y c.x, ///
    absorb(__ppml_break_fe=fe) separation(none) memory(fast) ///
    residual(__ppml_break_resid) xb(__ppml_break_xb) ///
    d(__ppml_break_d) eta(__ppml_break_eta) mu(__ppml_break_mu) ///
    dropreason(__ppml_break_drop) publicmode ///
    publiccmdline("feppml licensed Break prior") publictimeit publicdisplay ///
    inject(breakpostpublic)
local break_rc = _rc
assert c(processors) == `host_processors'
assert `break_rc' == 1
foreach output in __ppml_break_fe __ppml_break_resid __ppml_break_xb ///
    __ppml_break_d __ppml_break_eta __ppml_break_mu __ppml_break_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == `"`prior_cmd'"'
local after_N : display %21x e(N)
assert `"`after_N'"' == `"`prior_N'"'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_V') == 0
assert e(sample) == __ppml_prior_sample

* Null e() is a distinct hold/unhold state.  Repeat the genuine in-core Break,
* then retain the post-publication Break case as a separate ADO transaction
* boundary.  Both must remove every output, including the named saved FE.
ereturn clear
quietly datasignature set, reset
capture noisily _fereg_estimate_ppml y c.x, ///
    absorb(__ppml_native_null_fe=fe) separation(none) memory(fast) ///
    residual(__ppml_native_null_resid) xb(__ppml_native_null_xb) ///
    d(__ppml_native_null_d) eta(__ppml_native_null_eta) ///
    mu(__ppml_native_null_mu) dropreason(__ppml_native_null_drop) ///
    threads(`host_processors') inject(breaknative)
local native_null_break_rc = _rc
assert c(processors) == `host_processors'
assert `native_null_break_rc' == 1
foreach output in __ppml_native_null_fe __ppml_native_null_resid ///
    __ppml_native_null_xb __ppml_native_null_d __ppml_native_null_eta ///
    __ppml_native_null_mu __ppml_native_null_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == ""
assert missing(e(N))
capture confirm matrix e(feppml_receipt)
assert _rc == 111
quietly datasignature confirm

capture noisily _fereg_estimate_ppml y c.x, ///
    absorb(__ppml_null_fe=fe) separation(none) memory(fast) ///
    residual(__ppml_null_resid) xb(__ppml_null_xb) ///
    d(__ppml_null_d) eta(__ppml_null_eta) mu(__ppml_null_mu) ///
    dropreason(__ppml_null_drop) publicmode ///
    publiccmdline("feppml licensed Break null") publictimeit publicdisplay ///
    inject(breakpostpublic)
local null_break_rc = _rc
assert c(processors) == `host_processors'
assert `null_break_rc' == 1
foreach output in __ppml_null_fe __ppml_null_resid __ppml_null_xb ///
    __ppml_null_d __ppml_null_eta __ppml_null_mu __ppml_null_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == ""
assert missing(e(N))
quietly datasignature confirm

* Finally interrupt a real public prediction before the protected rename.  A
* large duplicated e(sample) keeps the private construction/validation window
* open long enough for the PTY controller without adding a developer option to
* the public predict syntax.
quietly _fereg_ppml y c.x, separation(none) memory(fast)
tempname predict_b predict_V
matrix `predict_b' = e(b)
matrix `predict_V' = e(V)
quietly expand 50000
display as result "FEPPML_" "PREDICT_BREAK_ARMED"
capture noisily predict double __ppml_predict_break, mu
local predict_break_rc = _rc
assert c(processors) == `host_processors'
assert `predict_break_rc' == 1
confirm new variable __ppml_predict_break
assert mreldif(e(b), `predict_b') == 0
assert mreldif(e(V), `predict_V') == 0

* Split the literal so the PTY controller cannot mistake command echo for the
* emitted terminal sentinel.
set processors `original_processors'
display as result "END_" "FIXTURE|estimate_ppml_eclass_break|rc=0"
