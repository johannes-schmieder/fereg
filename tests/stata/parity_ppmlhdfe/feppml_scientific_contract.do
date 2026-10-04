version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
clear all
set more off

* Finite nonnegative outcomes include fractional observations.  The two
* design-cell means identify (_cons, x) exactly as (0, ln(3)).
input long obs_id double(y x)
1 0.5 0
2 1.0 0
3 1.5 0
4 1.0 1
5 3.0 1
6 5.0 1
end

`feppml_test_command' y c.x, separation(none) vce(robust)
assert `"`e(cmd)'"' == "feppml"
assert e(converged) == 1
assert e(N_physical) == 6
assert e(N_effective) == 6
assert e(sample) == 1
assert abs(_b[_cons]) <= 1e-7
assert abs(_b[x] - ln(3)) <= 1e-7
assert e(deviance) >= 0 & e(deviance) < .
assert e(ll) < .
assert e(ll_0) < .
assert e(mu_0) > 0 & e(mu_0) < .
assert e(r2_p) < .
assert e(chi2) >= 0 & e(chi2) < .
assert e(df_chi2) == e(df_m)
assert _se[x] > 0 & _se[x] < .
assert reldif(e(chi2), (_b[x] / _se[x])^2) <= 1e-8
assert e(feppml_receipt)[1,1] == 5354602
assert e(feppml_receipt)[1,20] == 16
assert e(feppml_receipt)[1,21] == 14
assert e(feppml_receipt)[1,22] == 5
assert e(feppml_receipt)[1,33] == 1

* A no-FE fit retains no unrequested observation-level output, yet the full
* prediction surface is reconstructible from the current finite predictor.
assert e(N_hdfe) == 0
assert `"`e(d_var)'"' == ""
assert `"`e(eta_var)'"' == ""
assert `"`e(mu_var)'"' == ""
predict double __fit_only_xb, xb
predict double __fit_only_d, d
predict double __fit_only_eta, eta
predict double __fit_only_default
predict double __fit_only_mu, mu
predict double __fit_only_response, response
predict double __fit_only_score, score
predict double __fit_only_pearson, pearson
predict double __fit_only_working, working
predict double __fit_only_deviance, deviance
predict double __fit_only_anscombe, anscombe
assert abs(__fit_only_d) <= 1e-12 if e(sample)
assert abs(__fit_only_eta - __fit_only_xb) <= 1e-10 if e(sample)
assert reldif(__fit_only_default, exp(__fit_only_eta)) <= 1e-10 if e(sample)
assert reldif(__fit_only_mu, __fit_only_default) <= 1e-12 if e(sample)
assert abs(__fit_only_response - (y - __fit_only_mu)) <= 1e-10 if e(sample)
assert abs(__fit_only_score - __fit_only_response) <= 1e-12 if e(sample)
assert abs(__fit_only_pearson - ///
    (y - __fit_only_mu) / sqrt(__fit_only_mu)) <= 1e-10 if e(sample)
assert abs(__fit_only_working - ///
    (y - __fit_only_mu) / __fit_only_mu) <= 1e-10 if e(sample)
generate double __fit_only_dev_formula = 2 * ///
    (y * (log(y) - log(__fit_only_mu)) - (y - __fit_only_mu)) if e(sample)
assert abs(__fit_only_deviance - __fit_only_dev_formula) <= ///
    1e-9 * max(1, abs(__fit_only_dev_formula)) if e(sample)
generate double __fit_only_ans_formula = 1.5 * ///
    (y^(2/3) - __fit_only_mu^(2/3)) / __fit_only_mu^(1/6) if e(sample)
assert abs(__fit_only_anscombe - __fit_only_ans_formula) <= ///
    1e-10 * max(1, abs(__fit_only_ans_formula)) if e(sample)

* Deviance validates the current outcome domain and its source before the
* requested public target is committed.
replace y = -1 in 1
capture noisily predict double __negative_deviance if _n == 1, deviance
assert _rc == 430
confirm new variable __negative_deviance
replace y = 0.5 in 1
preserve
drop y
capture noisily predict double __deleted_y_deviance if _n == 1, deviance
assert _rc == 111
confirm new variable __deleted_y_deviance
restore

* With absorbed effects, a fit-only transaction still supports xb, but it
* may not hide an O(N) d/eta/mu snapshot solely for later prediction.
clear
set obs 60
generate long obs_id = _n
generate byte fe = 1 + mod(_n - 1, 5)
generate double x = mod(7 * _n, 29) / 14 - 1
generate double off = 0.01 * mod(_n, 4)
generate double y = exp(0.20 * x + 0.07 * fe + off) + 0.05 * mod(_n, 3)

`feppml_test_command' y c.x, absorb(fe) offset(off) separation(none) keepsingletons
assert e(N_hdfe) == 1
assert `"`e(d_var)'"' == ""
assert `"`e(eta_var)'"' == ""
assert `"`e(mu_var)'"' == ""
tempname fe_fit_b fe_fit_V
matrix `fe_fit_b' = e(b)
matrix `fe_fit_V' = e(V)
generate byte __fe_fit_sample = e(sample)

predict double __fe_fit_only_xb, xb
assert __fe_fit_only_xb < . if e(sample)

* Alias-pair errors are syntax failures and create no partial target.
capture noisily predict double __fe_alias_d, d absorbed
assert _rc == 198
confirm new variable __fe_alias_d
capture noisily predict double __fe_alias_eta, eta xbd
assert _rc == 198
confirm new variable __fe_alias_eta

* Every FE-dependent prediction fails atomically when no compatible output
* was retained.  The default request is mu and follows the same policy.
capture noisily predict double __fe_missing_default
assert _rc == 498
confirm new variable __fe_missing_default
foreach statistic in d absorbed eta xbd mu response score pearson working deviance anscombe {
    local target "__fe_missing_`statistic'"
    capture noisily predict double `target', `statistic'
    assert _rc == 498
    confirm new variable `target'
}
assert e(sample) == __fe_fit_sample
assert mreldif(e(b), `fe_fit_b') == 0
assert mreldif(e(V), `fe_fit_V') == 0

* The no-compatible-output policy is decided before touching current RHS or
* adjustment variables; absence therefore remains r(498), not an incidental
* variable-not-found error.
preserve
drop x off
capture noisily predict double __fe_missing_after_drop, eta
assert _rc == 498
confirm new variable __fe_missing_after_drop
restore

* Retained eta() and mu() are self-contained snapshot fallbacks.  They remain
* usable when current finite covariates and the adjustment variable no longer
* exist; only retained d() intentionally requires current-data reconstruction.
preserve
`feppml_test_command' y c.x, absorb(fe) offset(off) separation(none) ///
    keepsingletons eta(__snapshot_eta)
drop x off
predict double __eta_from_snapshot, eta
predict double __mu_from_eta_snapshot, mu
predict double __response_from_eta_snapshot, response
assert abs(__eta_from_snapshot - __snapshot_eta) <= 1e-12 if e(sample)
assert reldif(__mu_from_eta_snapshot, exp(__snapshot_eta)) <= 1e-10 if e(sample)
assert abs(__response_from_eta_snapshot - (y - __mu_from_eta_snapshot)) <= ///
    1e-10 if e(sample)
restore

preserve
`feppml_test_command' y c.x, absorb(fe) offset(off) separation(none) ///
    keepsingletons mu(__snapshot_mu)
drop x off
predict double __eta_from_mu_snapshot, eta
predict double __mu_from_snapshot, mu
assert abs(__eta_from_mu_snapshot - log(__snapshot_mu)) <= 1e-12 if e(sample)
assert reldif(__mu_from_snapshot, __snapshot_mu) <= 1e-10 if e(sample)
restore

* Retained d is authoritative when eta and mu were also requested.  Prediction
* combines it with current X and offset rather than copying stale snapshots.
`feppml_test_command' y c.x, absorb(fe) offset(off) separation(none) ///
    keepsingletons d(__retained_d) eta(__retained_eta) mu(__retained_mu)
assert `"`e(d_var)'"' == "__retained_d"
assert `"`e(eta_var)'"' == "__retained_eta"
assert `"`e(mu_var)'"' == "__retained_mu"
preserve
drop __retained_d
capture noisily predict double __deleted_d_prediction, d
assert _rc == 111
confirm new variable __deleted_d_prediction
restore
replace x = x + 0.35 if e(sample)
replace off = off - 0.20 if e(sample)
replace __retained_eta = __retained_eta + 8 if e(sample)
replace __retained_mu = __retained_mu * 13 if e(sample)

predict double __preferred_xb, xb
predict double __preferred_d, d
predict double __preferred_eta, eta
predict double __preferred_mu, mu
predict double __preferred_response, response
generate double __manual_current_xb = _b[_cons] + _b[x] * x + off if e(sample)
assert abs(__preferred_xb - __manual_current_xb) <= ///
    1e-10 * max(1, abs(__manual_current_xb)) if e(sample)
assert abs(__preferred_d - __retained_d) <= 1e-12 if e(sample)
assert abs(__preferred_eta - (__manual_current_xb + __retained_d)) <= ///
    1e-10 * max(1, abs(__preferred_eta)) if e(sample)
assert reldif(__preferred_mu, exp(__preferred_eta)) <= 1e-10 if e(sample)
assert abs(__preferred_response - (y - __preferred_mu)) <= 1e-10 if e(sample)
assert abs(__preferred_eta - __retained_eta) > 1 if e(sample)
assert reldif(__preferred_mu, __retained_mu) > 0.1 if e(sample)

* exposure() is exactly log(exposure) as an offset.
clear
set obs 6
generate long obs_id = _n
generate byte x = _n > 3
generate double exposure = cond(_n <= 3, _n, _n - 3)
generate double offset = log(exposure)
generate double y = exposure * cond(x, 3, 1)

`feppml_test_command' y c.x, exposure(exposure) separation(none)
tempname exposure_b exposure_V
matrix `exposure_b' = e(b)
matrix `exposure_V' = e(V)
generate byte __exposure_sample = e(sample)
assert abs(_b[_cons]) <= 1e-7
assert abs(_b[x] - ln(3)) <= 1e-7

`feppml_test_command' y c.x, offset(offset) separation(none)
assert mreldif(e(b), `exposure_b') <= 1e-10
assert mreldif(e(V), `exposure_V') <= 1e-9
assert e(sample) == __exposure_sample

* Invalid exposure and outcome domains fail atomically.
tempname accepted_b accepted_V
matrix `accepted_b' = e(b)
matrix `accepted_V' = e(V)
generate byte __accepted_sample = e(sample)

replace exposure = 0 in 2
capture noisily `feppml_test_command' y c.x, exposure(exposure) separation(none) ///
    residuals(__invalid_exposure_resid) mu(__invalid_exposure_mu)
assert _rc == 459
confirm new variable __invalid_exposure_resid
confirm new variable __invalid_exposure_mu
assert `"`e(cmd)'"' == "feppml"
assert e(sample) == __accepted_sample
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0
replace exposure = 2 in 2

generate double y_negative = y
replace y_negative = -0.25 in 2
capture noisily `feppml_test_command' y_negative c.x, separation(none) ///
    residuals(__negative_resid) eta(__negative_eta) mu(__negative_mu)
assert _rc == 459
confirm new variable __negative_resid
confirm new variable __negative_eta
confirm new variable __negative_mu
assert e(sample) == __accepted_sample
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0

generate double y_zero = 0
capture noisily `feppml_test_command' y_zero c.x, ///
    residuals(__zero_resid) xb(__zero_xb) d(__zero_d) ///
    eta(__zero_eta) mu(__zero_mu) dropreason(__zero_drop)
assert _rc == 2001
confirm new variable __zero_resid
confirm new variable __zero_xb
confirm new variable __zero_d
confirm new variable __zero_eta
confirm new variable __zero_mu
confirm new variable __zero_drop
assert `"`e(cmd)'"' == "feppml"
assert e(sample) == __accepted_sample
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0

* Candidate-sample ownership and stable drop-reason codes remain in Rust.
clear
set obs 14
generate long obs_id = _n
generate double y = 0.5 + mod(_n, 5)
generate double x = mod(_n, 4) - 1.5
generate double off = 0.01 * _n
generate double fe = 1 + mod(_n, 3)
generate double cl = 1 + mod(_n, 4)
generate double pweight = 0.5 + mod(_n, 5)
replace y = . in 2
replace x = . in 3
replace fe = . in 4
replace cl = . in 5
replace off = . in 6
replace pweight = . in 7
replace y = -1 in 13

`feppml_test_command' y c.x [pw=pweight] if obs_id <= 12, absorb(fe) vce(cluster cl) ///
    offset(off) separation(none) keepsingletons dropreason(__sample_drop)
assert e(N_full) == 14
assert e(N_physical) == 6
assert e(sample) == inlist(obs_id, 1, 8, 9, 10, 11, 12)
assert __sample_drop == 0 if e(sample)
assert __sample_drop == 2 if obs_id == 2
assert __sample_drop == 3 if obs_id == 3
assert __sample_drop == 5 if obs_id == 4
assert __sample_drop == 6 if obs_id == 5
assert __sample_drop == 7 if obs_id == 6
assert __sample_drop == 4 if obs_id == 7
assert __sample_drop == 1 if obs_id > 12

* Post-separation rank is deterministic: the first independent regressor is
* kept and the later exact duplicate is zero-filled in b and V.
clear
set obs 30
generate long obs_id = _n
generate double x = mod(_n, 7) - 3
generate double x_duplicate = 2 * x
generate double y = 0.5 + mod(3 * _n, 11)

`feppml_test_command' y c.x c.x_duplicate, separation(none)
local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "x o.x_duplicate _cons"
assert e(rank) == 2
assert e(df_m) == 1
assert e(feppml_status)[1,2] == 1
assert e(feppml_status)[2,2] == 6
assert e(feppml_status)[3,2] == 1
assert e(b)[1,2] == 0
forvalues index = 1/3 {
    assert e(V)[2,`index'] == 0
    assert e(V)[`index',2] == 0
}

* Canonical factor-base status remains distinct from a numerical omission.
generate byte factor_group = mod(_n, 3)
`feppml_test_command' y ib0.factor_group c.x, separation(none)
local base_slot 0
local factor_slots = colsof(e(b))
forvalues slot = 1/`factor_slots' {
    if e(feppml_status)[`slot',2] == 2 local base_slot `slot'
}
assert `base_slot' > 0
assert e(b)[1,`base_slot'] == 0
forvalues index = 1/`factor_slots' {
    assert e(V)[`base_slot',`index'] == 0
    assert e(V)[`index',`base_slot'] == 0
}

* Only fweights and pweights are public PPML weights.
generate double unsupported_weight = 1 + mod(_n, 3)
capture noisily `feppml_test_command' y c.x [aw=unsupported_weight], separation(none)
assert _rc == 101
capture noisily `feppml_test_command' y c.x [iw=unsupported_weight], separation(none)
assert _rc == 101

* The reported factor axis is the parser-owned candidate axis, not a second
* fvexpand on the final sample.  Level zero exists on the candidate row below
* but disappears when its missing outcome is excluded.  It must remain the
* declared base while the surviving levels retain their original active recipes.
clear
set obs 14
generate long obs_id = _n
generate byte factor_vanish = cond(_n == 1, 0, cond(_n <= 7, 1, 2))
generate double x = mod(_n, 4) - 1.5
generate double y = 0.5 + mod(3 * _n, 7)
replace y = . in 1

`feppml_test_command' y ib0.factor_vanish c.x, separation(none) keepsingletons
assert e(sample) == (obs_id > 1)
assert colsof(e(b)) == 5
assert rowsof(e(feppml_status)) == 5
assert e(feppml_status)[1,2] == 2
assert inlist(e(feppml_status)[2,2], 1, 6)
assert inlist(e(feppml_status)[3,2], 1, 6)
assert inlist(e(feppml_status)[5,2], 1, 6)
assert e(b)[1,1] == 0

* Fixed-width string FE and cluster sources are supported, while strL source
* identities fail closed without disturbing the accepted result or row order.
clear
set obs 40
generate long obs_id = _n
generate double x = mod(7 * _n, 17) / 8 - 1
generate double y = 0.5 + mod(11 * _n, 13) / 4
generate str8 string_fe = "fe" + string(1 + mod(_n, 5))
generate str8 string_cluster = "cl" + string(1 + mod(_n, 7))
`feppml_test_command' y c.x, absorb(string_fe) ///
    vce(cluster string_cluster) separation(none) keepsingletons
assert e(converged) == 1
assert e(N) == 40
tempname string_b string_V
matrix `string_b' = e(b)
matrix `string_V' = e(V)
estimates store __ppml_named
gsort -obs_id
generate long __sort_position = _n
generate strL long_fe = string_fe
capture noisily `feppml_test_command' y c.x, absorb(long_fe) ///
    separation(none) keepsingletons
assert _rc == 498
assert __sort_position == _n
assert mreldif(e(b), `string_b') == 0
assert mreldif(e(V), `string_V') == 0
generate strL long_cluster = string_cluster
capture noisily `feppml_test_command' y c.x, vce(cluster long_cluster) ///
    separation(none)
assert _rc == 498
assert __sort_position == _n
assert mreldif(e(b), `string_b') == 0
assert mreldif(e(V), `string_V') == 0
estimates restore __ppml_named
assert `"`e(cmd)'"' == "feppml"

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_scientific_contract|rc=0"
