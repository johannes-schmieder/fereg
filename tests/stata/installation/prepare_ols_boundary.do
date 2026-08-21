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

gen double aw = 1 + _n / 10
replace aw = -1 in 1
replace aw = 0 in 8
replace aw = . in 9
_stfixest_prepare_ols_test y c.x c.z if _n >= 2, ///
    aweight(aw) originalobs(_st_aw_original) retained(_st_aw_retained) ///
    dropreason(_st_aw_reason)
assert r(n_original) == 28
assert r(n_retained) == 21
assert "`r(weight_type)'" == "aweight"
assert "`r(weight_variable)'" == "aw"
assert _st_aw_original == _n
assert _st_aw_reason[1] == 1
assert _st_aw_reason[2] == 2
assert _st_aw_reason[5] == 3
assert _st_aw_reason[6] == 3
assert _st_aw_reason[7] == 3
assert _st_aw_reason[8] == 4
assert _st_aw_reason[9] == 4
assert _st_aw_retained == !inlist(_n, 1, 2, 5, 6, 7, 8, 9)

gen double aw_rescaled = (1 + _n / 10) * 10
gen double aw_huge = (1 + _n / 10) * 1e200
_stfixest_prepare_ols_test y c.x c.z, aweight(aw_rescaled) ///
    originalobs(_st_aw_scaled_original) retained(_st_aw_scaled_retained) ///
    dropreason(_st_aw_scaled_reason)
assert r(n_retained) == 24
assert _st_aw_scaled_retained == !inlist(_n, 2, 5, 6, 7)
_stfixest_prepare_ols_test y c.x c.z, aweight(aw_huge) ///
    originalobs(_st_aw_huge_original) retained(_st_aw_huge_retained) ///
    dropreason(_st_aw_huge_reason)
assert r(n_retained) == 24
assert _st_aw_huge_retained == _st_aw_scaled_retained

gen double aw_negative = 1
replace aw_negative = -1 in 10
capture noisily _stfixest_prepare_ols_test y c.x c.z, aweight(aw_negative) ///
    originalobs(_st_aw_bad_original) retained(_st_aw_bad_retained) ///
    dropreason(_st_aw_bad_reason)
assert _rc == 459
assert missing(_st_aw_bad_original)
assert missing(_st_aw_bad_retained)
assert missing(_st_aw_bad_reason)

preserve
clear
input double(y x fw_valid)
2.1 -1.2 1
2.7 -0.9 2
3.0 -0.6 3
3.8 -0.3 1
4.2  0.0 4
4.9  0.3 2
5.1  0.6 1
5.8  0.9 3
6.3  1.2 2
6.7  1.5 1
7.4  1.8 4
7.9  2.1 2
end

_stfixest_prepare_ols_test y c.x, fweight(fw_valid) ///
    originalobs(_st_fw_original) retained(_st_fw_retained) ///
    dropreason(_st_fw_reason)
assert r(n_retained) == 12
assert "`r(weight_type)'" == "fweight"
assert "`r(weight_variable)'" == "fw_valid"
assert _st_fw_original == _n
assert _st_fw_retained == 1
assert _st_fw_reason == 0

gen double fw_fractional = fw_valid
replace fw_fractional = 1.5 in 1
capture noisily _stfixest_prepare_ols_test y c.x, fweight(fw_fractional) ///
    originalobs(_st_fw_bad_original) retained(_st_fw_bad_retained) ///
    dropreason(_st_fw_bad_reason)
assert _rc == 459
assert missing(_st_fw_bad_original)
assert missing(_st_fw_bad_retained)
assert missing(_st_fw_bad_reason)

_stfixest_prepare_ols_test y c.x if _n >= 2, fweight(fw_fractional) ///
    originalobs(_st_fw_if_original) retained(_st_fw_if_retained) ///
    dropreason(_st_fw_if_reason)
assert r(n_retained) == 11
assert _st_fw_if_reason[1] == 1
assert _st_fw_if_retained == (_n >= 2)

capture noisily _stfixest_prepare_ols_test y c.x, ///
    aweight(fw_valid) fweight(fw_valid) ///
    originalobs(_st_mixed_original) retained(_st_mixed_retained) ///
    dropreason(_st_mixed_reason)
assert _rc == 198
confirm new variable _st_mixed_original
confirm new variable _st_mixed_retained
confirm new variable _st_mixed_reason
restore

capture noisily stfeols y x z
assert _rc == 498
capture noisily stfepois y x z
assert _rc == 498

di as result "prepare_ols_boundary.do: PASS"
di as result "END_FIXTURE|prepare_ols_boundary|rc=0"
