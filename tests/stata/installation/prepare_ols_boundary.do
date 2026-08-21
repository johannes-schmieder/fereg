version 18.0
clear all
set obs 28
gen double y = 1 + _n / 10
gen double x = cond(mod(_n, 2), -0, _n / 7)
gen double z = _n * 1e-200
replace y = . in 2
replace x = . in 5
replace z = . in 6
replace x = . in 7
replace z = . in 7

_stfixest_prepare_ols_test y c.x c.z, ///
    originalobs(_st_original) retained(_st_retained) dropreason(_st_reason)

assert r(n_original) == 28
assert r(n_retained) == 24
assert _st_original == _n
assert _st_retained == !inlist(_n, 2, 5, 6, 7)
assert _st_reason[2] == 2
assert _st_reason[5] == 3
assert _st_reason[6] == 3
assert _st_reason[7] == 3
assert _st_reason == 0 if !inlist(_n, 2, 5, 6, 7)

_stfixest_prepare_ols_test y, ///
    originalobs(_st_empty_original) retained(_st_empty_retained) ///
    dropreason(_st_empty_reason) noconstant
assert r(n_retained) == 27
assert _st_empty_reason[2] == 2

_stfixest_prepare_ols_test y if 0, ///
    originalobs(_st_none_original) retained(_st_none_retained) ///
    dropreason(_st_none_reason) noconstant
assert r(n_retained) == 0
assert _st_none_retained == 0
assert _st_none_reason == 1

capture noisily stfeols y x z
assert _rc == 498
capture noisily stfepois y x z
assert _rc == 498

di as result "prepare_ols_boundary.do: PASS"
di as result "END_FIXTURE|prepare_ols_boundary|rc=0"
