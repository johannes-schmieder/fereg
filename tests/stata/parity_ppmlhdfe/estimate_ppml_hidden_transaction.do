version 18.0
clear all
set more off

set obs 80
generate long obs_id = _n
generate double x = mod(_n, 11) / 5 - 1
generate double off = 0.02 * mod(_n, 4)
generate double exposure = exp(off)
generate double fw = 1 + mod(_n, 3)
generate double pw = 0.5 + mod(_n, 7) / 3
generate double y = mod(_n, 5)
generate byte fe = 1 + mod(_n - 1, 8)
replace y = y + 1 if mod(_n, 13) == 0

quietly regress y x
tempname before_b before_v
matrix `before_b' = e(b)
matrix `before_v' = e(V)
generate byte __before_sample = e(sample)

* IID/OLS aliases are not public PPML inference.  Reject them during public
* option validation, before the substantive plugin call, without disturbing
* prior estimation state or the requested-output namespace.
foreach unsupported_vce in iid ols {
    local output_prefix "__unsupported_`unsupported_vce'"
    capture noisily _fereg_ppml y c.x, absorb(`output_prefix'_fe=fe) ///
        vce(`unsupported_vce') residuals(`output_prefix'_resid) ///
        xb(`output_prefix'_xb) d(`output_prefix'_d) ///
        eta(`output_prefix'_eta) mu(`output_prefix'_mu) ///
        dropreason(`output_prefix'_drop)
    assert _rc == 198
    foreach suffix in fe resid xb d eta mu drop {
        confirm new variable `output_prefix'_`suffix'
    }
    assert `"`e(cmd)'"' == "regress"
    assert e(sample) == __before_sample
    assert mreldif(e(b), `before_b') == 0
    assert mreldif(e(V), `before_v') == 0
}

* Developer injections are not exposed on the public adapter.
capture noisily _fereg_ppml y c.x, inject(panic) residuals(__public_inject)
assert _rc == 198
confirm new variable __public_inject
assert `"`e(cmd)'"' == "regress"
assert e(sample) == __before_sample
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_v') == 0

* Public metadata is posted inside the same guarded transaction.  A failure
* after that metadata must restore prior e() and every requested output.
capture noisily _fereg_estimate_ppml y c.x, absorb(__public_tail_fe=fe) ///
    memory(fast) separation(none) residual(__public_tail_resid) ///
    xb(__public_tail_xb) d(__public_tail_d) eta(__public_tail_eta) ///
    mu(__public_tail_mu) dropreason(__public_tail_drop) ///
    publicmode publiccmdline("feppml guarded tail") publictimeit publicdisplay ///
    inject(postpublic)
assert _rc == 3498
foreach output in __public_tail_fe __public_tail_resid __public_tail_xb ///
    __public_tail_d __public_tail_eta __public_tail_mu __public_tail_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == "regress"
assert e(sample) == __before_sample
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_v') == 0

foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics precommit {
    capture noisily _fereg_estimate_ppml y c.x, absorb(__failed_fe=fe) memory(fast) ///
        separation(none) residual(__failed_resid) eta(__failed_eta) ///
        xb(__failed_xb) d(__failed_d) mu(__failed_mu) ///
        dropreason(__failed_drop) inject(`stage')
    assert _rc == 3498
    confirm new variable __failed_resid
    confirm new variable __failed_eta
    confirm new variable __failed_xb
    confirm new variable __failed_d
    confirm new variable __failed_mu
    confirm new variable __failed_drop
    confirm new variable __failed_fe
    assert `"`e(cmd)'"' == "regress"
    assert e(sample) == __before_sample
    assert mreldif(e(b), `before_b') == 0
    assert mreldif(e(V), `before_v') == 0
}

capture noisily _fereg_estimate_ppml y c.x, absorb(__panic_fe=fe) memory(fast) ///
    separation(none) residual(__panic_resid) inject(panic)
assert _rc == 3499
confirm new variable __panic_resid
confirm new variable __panic_fe
assert `"`e(cmd)'"' == "regress"
assert e(sample) == __before_sample
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_v') == 0

* Malformed native publications are not user data errors.  A broken wire
* header is r(499); semantic certificates and output maps are internal r(3499).
foreach mutation in mutatewire mutatesemantic mutatesolver mutateoutput {
    local expected_rc = cond(`"`mutation'"' == "mutatewire", 499, 3499)
    capture noisily _fereg_estimate_ppml y c.x, absorb(__class_fe=fe) memory(fast) ///
        separation(none) residual(__class_resid) eta(__class_eta) ///
        xb(__class_xb) d(__class_d) mu(__class_mu) ///
        dropreason(__class_drop) inject(`mutation')
    assert _rc == `expected_rc'
    confirm new variable __class_resid
    confirm new variable __class_eta
    confirm new variable __class_xb
    confirm new variable __class_d
    confirm new variable __class_mu
    confirm new variable __class_drop
    confirm new variable __class_fe
    assert `"`e(cmd)'"' == "regress"
    assert e(sample) == __before_sample
    assert mreldif(e(b), `before_b') == 0
    assert mreldif(e(V), `before_v') == 0
}

* Every non-Break failure stage must also restore a genuinely null e() state.
* Request the complete observation-output surface and one saved FE so a single
* stale rename cannot hide behind the populated-e() rollback checks above.
ereturn clear
quietly datasignature set, reset
foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics precommit {
    capture noisily _fereg_estimate_ppml y c.x, absorb(__null_fe=fe) memory(fast) ///
        separation(none) residual(__null_resid) eta(__null_eta) ///
        xb(__null_xb) d(__null_d) mu(__null_mu) ///
        dropreason(__null_drop) inject(`stage')
    assert _rc == 3498
    confirm new variable __null_resid
    confirm new variable __null_eta
    confirm new variable __null_xb
    confirm new variable __null_d
    confirm new variable __null_mu
    confirm new variable __null_drop
    confirm new variable __null_fe
    assert `"`e(cmd)'"' == ""
    assert missing(e(N))
    quietly datasignature confirm
}

* Public metadata and timeit are part of the guarded transaction for a null
* e() state too; failure at the adapter tail must not create a result or leave
* any requested output behind.
capture noisily _fereg_estimate_ppml y c.x, absorb(__null_public_fe=fe) ///
    memory(fast) separation(none) residual(__null_public_resid) ///
    xb(__null_public_xb) d(__null_public_d) eta(__null_public_eta) ///
    mu(__null_public_mu) dropreason(__null_public_drop) ///
    publicmode publiccmdline("feppml guarded null tail") ///
    publictimeit publicdisplay inject(postpublic)
assert _rc == 3498
foreach output in __null_public_fe __null_public_resid __null_public_xb ///
    __null_public_d __null_public_eta __null_public_mu __null_public_drop {
    confirm new variable `output'
}
assert `"`e(cmd)'"' == ""
assert missing(e(N))
quietly datasignature confirm

_fereg_estimate_ppml y c.x, memory(fast) separation(none) ///
    residual(__ppml_resid) xb(__ppml_xb) d(__ppml_d) ///
    eta(__ppml_eta) mu(__ppml_mu) ///
    dropreason(__ppml_drop)
assert `"`e(cmd)'"' == "_fereg_estimate_ppml"
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
assert e(converged) == 1
assert e(N_physical) == _N
assert e(N_effective) == _N
capture confirm scalar e(raw_sumweights)
assert _rc == 111
assert e(weight_multiplier) == 1
capture confirm scalar e(log_raw_sumweights)
assert _rc == 111
assert e(log_weight_multiplier) == 0
assert e(feppml_objective_scale_code) == 1
assert e(feppml_allocation_audit_present) == 0
assert e(feppml_two_consecutive) == 1
assert e(feppml_rank_frozen) == 1
confirm scalar e(feppml_action_events_start)
confirm scalar e(feppml_action_events_end)
confirm scalar e(feppml_action_events_delta)
assert e(feppml_action_events_end) >= e(feppml_action_events_start)
assert e(feppml_action_events_delta) == ///
    e(feppml_action_events_end) - e(feppml_action_events_start)
mata: assert(all(st_matrix("e(feppml_runtime)")[|83,1 \ 95,1|] :< .))
assert e(sample) == 1
assert abs(__ppml_resid - (y - __ppml_mu)) < 1e-10
assert abs(__ppml_eta - log(__ppml_mu)) < 1e-10
assert abs(__ppml_eta - __ppml_xb - __ppml_d) < 1e-10
assert abs(__ppml_d) < 1e-12
assert __ppml_mu > 0 & __ppml_mu < .
assert __ppml_drop == 0
local names : colfullnames e(b)
assert `"`names'"' == "x _cons"

* The successful private public adapter exposes only metadata committed by
* the guarded transaction; it has no ereturn tail of its own.
drop __ppml_resid __ppml_xb __ppml_d __ppml_eta __ppml_mu __ppml_drop
_fereg_ppml y c.x, memory(fast) separation(none) ///
    residuals(__public_resid) xb(__public_xb) d(__public_d) ///
    eta(__public_eta) mu(__public_mu) dropreason(__public_drop)
assert `"`e(cmd)'"' == "feppml"
assert `"`e(predict)'"' == "feppml_p"
assert `"`e(feppml_gate)'"' == "public_ppml_v1"
assert `"`e(feppml_memory_requested)'"' == "fast"
assert e(feppml_poolsize) == 10
assert e(feppml_requested_tolerance) == 1e-8
assert e(feppml_iterate) == 10000
assert e(feppml_timeit) == 0

* Prediction is a small transaction too: a current-data mean representable as
* double but not float must fail without leaving the requested target.
assert abs(_b[x]) > 1e-12
replace x = (100 - _b[_cons]) / _b[x] in 1
capture noisily feppml_p float __ppml_overflow if _n == 1, mu
assert _rc == 430
confirm new variable __ppml_overflow

* Preserve the parser-owned original coefficient axis when a factor level is
* present in the candidate data but disappears during outcome filtering.
preserve
clear
set obs 40
generate byte group = cond(_n == 1, 0, cond(_n <= 20, 1, 2))
generate double y = 1 + 2 * (group == 2) + mod(_n, 4) / 10
replace y = . in 1
_fereg_estimate_ppml y ib0.group, memory(fast) separation(none)
local factor_names : colfullnames e(b)
assert colsof(e(b)) == 4
assert `"`factor_names'"' == "0b.group 1o.group 2.group _cons"
assert e(feppml_status)[1,2] == 2
assert e(feppml_status)[2,2] == 3
assert e(feppml_status)[3,2] == 1
assert e(feppml_status)[4,2] == 1
assert e(N_physical) == 39
restore

* A singular tested covariance has a positive tested-column inventory but no
* certified scalar Wald statistic.  The public contract omits both chi2
* scalars instead of publishing Stata missing values as if they were results.
preserve
replace y = 1
_fereg_estimate_ppml y c.x, memory(fast) separation(none)
assert e(df_m) == 1
capture confirm scalar e(chi2)
assert _rc == 111
capture confirm scalar e(df_chi2)
assert _rc == 111
restore

capture noisily _fereg_estimate_ppml y c.x, offset(off) exposure(exposure) ///
    memory(fast) separation(none)
assert _rc == 198

* Saved FE positions are a declaration-order mask over all FE channels.  A
* partial multiway request must publish the named first channel—not shift it
* onto the next requested channel—and may not change d or the estimate.
clear
set obs 48
generate long obs_id = _n
generate byte fe1 = ceil(_n / 12)
generate byte fe2 = 1 + mod(_n - 1, 6)
generate double x = mod(5 * _n, 17) / 8 - 1
generate double y = exp(0.2 * x + 0.1 * fe1 - 0.05 * fe2)

_fereg_estimate_ppml y c.x, absorb(__partial_fe1=fe1 fe2) ///
    memory(fast) separation(none) d(__partial_d) eta(__partial_eta)
assert e(feppml_receipt)[1,11] == 2
assert e(feppml_receipt)[1,18] == 1
bysort fe1 (obs_id): assert abs(__partial_fe1 - __partial_fe1[1]) <= 1e-10
tempname partial_b partial_V
matrix `partial_b' = e(b)
matrix `partial_V' = e(V)
clonevar __partial_fe1_copy = __partial_fe1
clonevar __partial_d_copy = __partial_d
clonevar __partial_eta_copy = __partial_eta
generate byte __partial_sample = e(sample)
drop __partial_fe1 __partial_d __partial_eta

_fereg_estimate_ppml y c.x, ///
    absorb(__all_fe1=fe1 __all_fe2=fe2) ///
    memory(fast) separation(none) d(__all_d) eta(__all_eta)
assert e(feppml_receipt)[1,11] == 2
assert e(feppml_receipt)[1,18] == 2
assert e(sample) == __partial_sample
assert mreldif(e(b), `partial_b') <= 1e-10
assert mreldif(e(V), `partial_V') <= 1e-9
assert abs(__all_fe1 - __partial_fe1_copy) <= 1e-10 if e(sample)
assert abs(__all_d - __partial_d_copy) <= 1e-10 if e(sample)
assert abs(__all_eta - __partial_eta_copy) <= 1e-10 if e(sample)
bysort fe2 (obs_id): assert abs(__all_fe2 - __all_fe2[1]) <= 1e-10

display as result "END_FIXTURE|estimate_ppml_hidden_transaction|rc=0"
