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

_stfixest_prepare_ols_test y c.x c.z, mapcheck ///
    originalobs(_st_map_original) retained(_st_map_retained) ///
    dropreason(_st_map_reason)
assert r(map_checked) == 1
assert r(n_retained) == 24
assert _st_map_retained == _st_retained
assert _st_map_reason == _st_reason

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

capture noisily _stfixest_prepare_ols_test y if 0, mapcheck ///
    originalobs(_st_none_map_original) retained(_st_none_map_retained) ///
    dropreason(_st_none_map_reason) noconstant
assert _rc == 459
assert missing(_st_none_map_original)
assert missing(_st_none_map_retained)
assert missing(_st_none_map_reason)

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

gen double pw = 0.5 + _n / 20
replace pw = -1 in 1
replace pw = . in 9
_stfixest_prepare_ols_test y c.x c.z if _n >= 2, pweight(pw) ///
    originalobs(_st_pw_original) retained(_st_pw_retained) ///
    dropreason(_st_pw_reason)
assert r(n_original) == 28
assert r(n_retained) == 22
assert "`r(weight_type)'" == "pweight"
assert "`r(weight_variable)'" == "pw"
assert _st_pw_original == _n
assert _st_pw_reason[1] == 1
assert _st_pw_reason[2] == 2
assert _st_pw_reason[5] == 3
assert _st_pw_reason[6] == 3
assert _st_pw_reason[7] == 3
assert _st_pw_reason[9] == 4
assert _st_pw_retained == !inlist(_n, 1, 2, 5, 6, 7, 9)

gen double pw_zero = 1
replace pw_zero = 0 in 10
capture noisily _stfixest_prepare_ols_test y c.x c.z, pweight(pw_zero) ///
    originalobs(_st_pw_bad_original) retained(_st_pw_bad_retained) ///
    dropreason(_st_pw_bad_reason)
assert _rc == 459
assert missing(_st_pw_bad_original)
assert missing(_st_pw_bad_retained)
assert missing(_st_pw_bad_reason)

capture noisily _stfixest_prepare_ols_test y c.x c.z, iweight(aw) ///
    originalobs(_st_iw_original) retained(_st_iw_retained) ///
    dropreason(_st_iw_reason)
assert _rc == 198
confirm new variable _st_iw_original
confirm new variable _st_iw_retained
confirm new variable _st_iw_reason

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
    aweight(fw_valid) fweight(fw_valid) pweight(fw_valid) ///
    originalobs(_st_mixed_original) retained(_st_mixed_retained) ///
    dropreason(_st_mixed_reason)
assert _rc == 198
confirm new variable _st_mixed_original
confirm new variable _st_mixed_retained
confirm new variable _st_mixed_reason
restore

gen double g = mod(_n, 3) + 1
replace g = -1 in 1
replace g = . in 10
_stfixest_prepare_ols_test y c.g ib2.g if _n >= 3, ///
    originalobs(_st_factor_original) retained(_st_factor_retained) ///
    dropreason(_st_factor_reason)
assert r(n_original) == 28
assert r(n_retained) == 25
assert _st_factor_original == _n
assert _st_factor_reason == 1 in 1/2
assert _st_factor_reason[10] == 3
assert _st_factor_retained == (_n >= 3 & _n != 10)

_stfixest_prepare_ols_test y ibn.g if _n >= 3, ///
    originalobs(_st_factor_bn_original) retained(_st_factor_bn_retained) ///
    dropreason(_st_factor_bn_reason)
assert r(n_retained) == 25
assert _st_factor_bn_reason == _st_factor_reason
assert _st_factor_bn_retained == _st_factor_retained

gen double h = mod(floor((_n - 1) / 3), 2) + 1
replace h = . in 11
_stfixest_prepare_ols_test y ib2.g##ib1.h if _n >= 3, ///
    originalobs(_st_interaction_original) retained(_st_interaction_retained) ///
    dropreason(_st_interaction_reason)
assert r(n_retained) == 24
assert _st_interaction_original == _n
assert _st_interaction_reason == 1 in 1/2
assert _st_interaction_reason[10] == 3
assert _st_interaction_reason[11] == 3
assert _st_interaction_retained == (_n >= 3 & !inlist(_n, 10, 11))

capture noisily _stfixest_prepare_ols_test y i.g#c.x if _n >= 3, ///
    originalobs(_st_mixed_factor_original) retained(_st_mixed_factor_retained) ///
    dropreason(_st_mixed_factor_reason)
assert _rc == 498
assert missing(_st_mixed_factor_original)
assert missing(_st_mixed_factor_retained)
assert missing(_st_mixed_factor_reason)

gen double fe_fractional = _n / 2 - 8
replace fe_fractional = . in 12
_stfixest_prepare_ols_test y, absorb(fe_fractional) noconstant keepsingletons ///
    originalobs(_st_fe_original) retained(_st_fe_retained) ///
    dropreason(_st_fe_reason)
assert r(n_original) == 28
assert r(n_retained) == 26
assert _st_fe_original == _n
assert _st_fe_reason[2] == 2
assert _st_fe_reason[12] == 5
assert _st_fe_retained == !inlist(_n, 2, 12)

_stfixest_prepare_ols_test y ib2.g if _n >= 3, absorb(g h g) keepsingletons ///
    originalobs(_st_shared_fe_original) retained(_st_shared_fe_retained) ///
    dropreason(_st_shared_fe_reason)
assert r(n_retained) == 24
assert _st_shared_fe_reason == 1 in 1/2
assert _st_shared_fe_reason[10] == 3
assert _st_shared_fe_reason[11] == 5
assert _st_shared_fe_retained == (_n >= 3 & !inlist(_n, 10, 11))

gen double fe_all_missing = .
_stfixest_prepare_ols_test y, absorb(fe_all_missing) noconstant ///
    originalobs(_st_all_fe_original) retained(_st_all_fe_retained) ///
    dropreason(_st_all_fe_reason)
assert r(n_retained) == 0
assert _st_all_fe_reason[2] == 2
assert _st_all_fe_reason == 5 if _n != 2
assert _st_all_fe_retained == 0

preserve
clear
input double(y x1 worker firm year)
1.2 -1.0 1 1 2020
1.8 -0.5 1 2 2021
2.4  0.0 2 2 2020
3.1  0.5 2 3 2021
3.7  1.0 3 3 2020
4.0 -0.8 4 4 2020
4.5 -0.2 4 5 2021
5.0  0.3 5 4 2020
5.6  0.9 5 5 2021
end

_stfixest_prepare_ols_test y c.x1, absorb(worker firm) ///
    originalobs(_st_singleton_original) retained(_st_singleton_retained) ///
    dropreason(_st_singleton_reason)
assert r(n_retained) == 4
assert r(n_singletons) == 5
assert r(keep_singletons) == 0
assert _st_singleton_original == _n
assert _st_singleton_retained == (_n >= 6)
assert _st_singleton_reason == 8 in 1/5
assert _st_singleton_reason == 0 in 6/9

_stfixest_prepare_ols_test y c.x1, absorb(worker firm) mapcheck ///
    originalobs(_st_fe_map_original) retained(_st_fe_map_retained) ///
    dropreason(_st_fe_map_reason)
assert r(map_checked) == 1
assert r(n_retained) == 4
assert _st_fe_map_retained == _st_singleton_retained
assert _st_fe_map_reason == _st_singleton_reason

_stfixest_prepare_ols_test y c.x1, absorb(worker firm) keepsingletons ///
    originalobs(_st_keep_singleton_original) retained(_st_keep_singleton_retained) ///
    dropreason(_st_keep_singleton_reason)
assert r(n_retained) == 9
assert r(n_singletons) == 0
assert r(keep_singletons) == 1
assert _st_keep_singleton_original == _n
assert _st_keep_singleton_retained == 1
assert _st_keep_singleton_reason == 0

_stfixest_prepare_ols_test y c.x1, absorb(worker firm) aweight(year) ///
    originalobs(_st_aw_singleton_original) retained(_st_aw_singleton_retained) ///
    dropreason(_st_aw_singleton_reason)
assert r(n_retained) == 4
assert r(n_singletons) == 5
assert _st_aw_singleton_retained == _st_singleton_retained
assert _st_aw_singleton_reason == _st_singleton_reason
restore

preserve
clear
input double(y group fw)
1 10 2
2 20 1
end
_stfixest_prepare_ols_test y, absorb(group) fweight(fw) noconstant ///
    originalobs(_st_fw_singleton_original) retained(_st_fw_singleton_retained) ///
    dropreason(_st_fw_singleton_reason)
assert r(n_retained) == 1
assert r(n_singletons) == 1
assert _st_fw_singleton_retained[1] == 1
assert _st_fw_singleton_reason[1] == 0
assert _st_fw_singleton_retained[2] == 0
assert _st_fw_singleton_reason[2] == 8
restore

capture noisily _stfixest_prepare_ols_test y, absorb(g#h) noconstant ///
    originalobs(_st_fe_interaction_original) retained(_st_fe_interaction_retained) ///
    dropreason(_st_fe_interaction_reason)
assert _rc == 498
assert missing(_st_fe_interaction_original)
assert missing(_st_fe_interaction_retained)
assert missing(_st_fe_interaction_reason)

capture noisily _stfixest_prepare_ols_test y, absorb(g##c.x) noconstant ///
    originalobs(_st_fe_slope_original) retained(_st_fe_slope_retained) ///
    dropreason(_st_fe_slope_reason)
assert _rc == 498
assert missing(_st_fe_slope_original)
assert missing(_st_fe_slope_retained)
assert missing(_st_fe_slope_reason)

capture noisily _stfixest_prepare_ols_test y, absorb(saved_fe=g) noconstant ///
    originalobs(_st_fe_saved_original) retained(_st_fe_saved_retained) ///
    dropreason(_st_fe_saved_reason)
assert _rc == 498
assert missing(_st_fe_saved_original)
assert missing(_st_fe_saved_retained)
assert missing(_st_fe_saved_reason)

gen str4 fe_string = cond(mod(_n, 2), "odd", "even")
capture noisily _stfixest_prepare_ols_test y c.x, absorb(fe_string) noconstant ///
    originalobs(_st_fe_string_original) retained(_st_fe_string_retained) ///
    dropreason(_st_fe_string_reason)
assert _rc == 498
confirm new variable _st_fe_string_original
confirm new variable _st_fe_string_retained
confirm new variable _st_fe_string_reason

capture noisily stfeols y x z
assert _rc == 498
capture noisily stfepois y x z
assert _rc == 498

di as result "prepare_ols_boundary.do: PASS"
di as result "END_FIXTURE|prepare_ols_boundary|rc=0"
