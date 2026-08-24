version 18.0
clear all
set more off

quietly findfile stfixest.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/stfixest.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* Accepted source-bound oracle:
* .ci/reference-probes/fe-interaction/e81b942c0dc0b122258489a60c956cdbe0121512.json
* case: receipt_driven_cases
* payload_sha256: 1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955
* This fixture consumes the committed sealed receipt; it does not invoke reghdfe.

// Sealed receipt case: missing_interaction_components
clear
set obs 12
generate long obs_id = 200 + _n
generate double y = real(word("3.10 3.90 4.40 5.20 5.80 6.50 7.10 7.90 8.40 9.00 . 9.80", _n))
generate double x = real(word("-1.20 -0.70 -0.20 0.30 0.80 1.30 1.80 2.30 -1.00 -0.50 0.00 0.50", _n))
generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 3 3", _n))
generate int firm = real(word("10 10 20 20 10 10 20 20 . 30 30 30", _n))
generate int year = real(word("2020 2020 2021 2021 2020 2020 2021 2021 2020 . 2022 2022", _n))
generate byte __stfx_expected_sample = 0
generate double __stfx_expected_resid = .
replace __stfx_expected_sample = 1 if inlist(obs_id, 201, 202, 203, 204, 205, 206, 207, 208)
replace __stfx_expected_sample = 1 if inlist(obs_id, 212)
replace __stfx_expected_resid = +1.2f98fe2d949feX-033 if obs_id == 201
replace __stfx_expected_resid = +1.9999999999938X-006 if obs_id == 202
replace __stfx_expected_resid = -1.9999999999928X-006 if obs_id == 203
replace __stfx_expected_resid = -1.2f98fe2d949feX-033 if obs_id == 204
replace __stfx_expected_resid = +1.9999999999928X-006 if obs_id == 205
replace __stfx_expected_resid = -1.99999999999e9X-005 if obs_id == 206
replace __stfx_expected_resid = -1.d843527fcabfcX-033 if obs_id == 207
replace __stfx_expected_resid = +1.9999999999a14X-006 if obs_id == 208
replace __stfx_expected_resid = +0.0000000000000X-3ff if obs_id == 212
_stfixest_estimate_ols_test y c.x, absorb(worker firm#year) keepsingletons residual(__stfx_reference_resid)
assert e(N) == (+1.2000000000000X+003)
assert e(N_full) == (+1.2000000000000X+003)
assert e(N_hdfe) == (+1.0000000000000X+001)
assert e(df_a) == (+1.0000000000000X+002)
assert e(df_m) == (+1.0000000000000X+000)
assert e(df_r) == (+1.0000000000000X+002)
assert e(num_singletons) == (+0.0000000000000X-3ff)
assert e(sumweights) == (+1.2000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker firm#year"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"ols"' 
matrix __stfx_b = e(b)
assert rowsof(__stfx_b) == 1
assert colsof(__stfx_b) == 2
local __stfx_names : rowfullnames __stfx_b
assert `"`__stfx_names'"' == `"y1"' 
local __stfx_names : colfullnames __stfx_b
assert `"`__stfx_names'"' == `"x _cons"' 
assert abs(__stfx_b[1,1] - (+1.8ccccccccccd1X+000)) <= 5e-9 * max(abs(+1.8ccccccccccd1X+000), 1)
assert abs(__stfx_b[1,2] - (+1.47db97530eca8X+002)) <= 5e-9 * max(abs(+1.47db97530eca8X+002), 1)
matrix __stfx_V = e(V)
assert rowsof(__stfx_V) == 2
assert colsof(__stfx_V) == 2
local __stfx_names : rowfullnames __stfx_V
assert `"`__stfx_names'"' == `"x _cons"' 
local __stfx_names : colfullnames __stfx_V
assert `"`__stfx_names'"' == `"x _cons"' 
assert abs(__stfx_V[1,1] - (+1.47ae147ae1493X-009)) <= 5e-6 * max(abs(+1.47ae147ae1493X-009), 1e-10)
assert abs(__stfx_V[1,2] - (-1.64ce9ed572779X-00a)) <= 5e-6 * max(abs(-1.64ce9ed572779X-00a), 1e-10)
assert abs(__stfx_V[2,1] - (-1.64ce9ed572779X-00a)) <= 5e-6 * max(abs(-1.64ce9ed572779X-00a), 1e-10)
assert abs(__stfx_V[2,2] - (+1.cd5750cad6873X-00b)) <= 5e-6 * max(abs(+1.cd5750cad6873X-00b), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 2
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.firm#1.year"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.8000000000000X+001)) <= 5e-9 * max(abs(+1.8000000000000X+001), 1)
assert abs(__stfx_D[1,2] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,3] - (+1.8000000000000X+001)) <= 5e-9 * max(abs(+1.8000000000000X+001), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.8000000000000X+001)) <= 5e-9 * max(abs(+1.8000000000000X+001), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,3] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
assert abs(__stfx_D[2,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
matrix __stfx_S = e(stfixest_status)
assert rowsof(__stfx_S) == 2
assert colsof(__stfx_S) == 4
assert __stfx_S[1,2] == 1
assert __stfx_S[2,2] == 5
generate byte __stfx_actual_sample = e(sample)
assert __stfx_actual_sample == __stfx_expected_sample
assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1

// Sealed receipt case: repeated_identical_interaction
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear
generate byte __stfx_expected_sample = 0
generate double __stfx_expected_resid = .
replace __stfx_expected_sample = 1 if inlist(obs_id, 3, 4, 11, 12, 15, 16, 23, 24)
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 3
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 4
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 11
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 12
replace __stfx_expected_resid = +1.9999bfffffff3X-005 if obs_id == 15
replace __stfx_expected_resid = -1.9999bfffffff3X-005 if obs_id == 16
replace __stfx_expected_resid = -1.9999bffffff7fX-005 if obs_id == 23
replace __stfx_expected_resid = +1.9999bffffff7fX-005 if obs_id == 24
_stfixest_estimate_ols_test y c.x1, absorb(worker firm#year firm#year) residual(__stfx_reference_resid)
assert e(N) == (+1.0000000000000X+003)
assert e(N_full) == (+1.8000000000000X+004)
assert e(N_hdfe) == (+1.8000000000000X+001)
assert e(df_a) == (+1.8000000000000X+002)
assert e(df_m) == (+0.0000000000000X-3ff)
assert e(df_r) == (+1.0000000000000X+001)
assert e(num_singletons) == (+1.0000000000000X+004)
assert e(sumweights) == (+1.0000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker firm#year firm#year"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"ols"' 
matrix __stfx_b = e(b)
assert rowsof(__stfx_b) == 1
assert colsof(__stfx_b) == 2
local __stfx_names : rowfullnames __stfx_b
assert `"`__stfx_names'"' == `"y1"' 
local __stfx_names : colfullnames __stfx_b
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_b[1,1] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_b[1,2] - (+1.b99999c000000X+002)) <= 5e-9 * max(abs(+1.b99999c000000X+002), 1)
matrix __stfx_V = e(V)
assert rowsof(__stfx_V) == 2
assert colsof(__stfx_V) == 2
local __stfx_names : rowfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
local __stfx_names : colfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_V[1,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[1,2] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,2] - (+1.47ae1eb853fc5X-00a)) <= 5e-6 * max(abs(+1.47ae1eb853fc5X-00a), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 3
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.firm#1.year 1.firm#1.year"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,2] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,3] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,3] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[3,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[3,2] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[3,3] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[3,4] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
assert abs(__stfx_D[3,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
matrix __stfx_S = e(stfixest_status)
assert rowsof(__stfx_S) == 2
assert colsof(__stfx_S) == 4
assert __stfx_S[1,2] == 3
assert __stfx_S[2,2] == 5
generate byte __stfx_actual_sample = e(sample)
assert __stfx_actual_sample == __stfx_expected_sample
assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1

// Sealed receipt case: interaction_order_year_firm
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear
generate byte __stfx_expected_sample = 0
generate double __stfx_expected_resid = .
replace __stfx_expected_sample = 1 if inlist(obs_id, 3, 4, 11, 12, 15, 16, 23, 24)
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 3
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 4
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 11
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 12
replace __stfx_expected_resid = +1.9999bfffffff3X-005 if obs_id == 15
replace __stfx_expected_resid = -1.9999bfffffff3X-005 if obs_id == 16
replace __stfx_expected_resid = -1.9999bffffff7fX-005 if obs_id == 23
replace __stfx_expected_resid = +1.9999bffffff7fX-005 if obs_id == 24
_stfixest_estimate_ols_test y c.x1, absorb(worker year#firm) residual(__stfx_reference_resid)
assert e(N) == (+1.0000000000000X+003)
assert e(N_full) == (+1.8000000000000X+004)
assert e(N_hdfe) == (+1.0000000000000X+001)
assert e(df_a) == (+1.8000000000000X+002)
assert e(df_m) == (+0.0000000000000X-3ff)
assert e(df_r) == (+1.0000000000000X+001)
assert e(num_singletons) == (+1.0000000000000X+004)
assert e(sumweights) == (+1.0000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker year#firm"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"ols"' 
matrix __stfx_b = e(b)
assert rowsof(__stfx_b) == 1
assert colsof(__stfx_b) == 2
local __stfx_names : rowfullnames __stfx_b
assert `"`__stfx_names'"' == `"y1"' 
local __stfx_names : colfullnames __stfx_b
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_b[1,1] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_b[1,2] - (+1.b99999c000000X+002)) <= 5e-9 * max(abs(+1.b99999c000000X+002), 1)
matrix __stfx_V = e(V)
assert rowsof(__stfx_V) == 2
assert colsof(__stfx_V) == 2
local __stfx_names : rowfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
local __stfx_names : colfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_V[1,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[1,2] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,2] - (+1.47ae1eb853fc5X-00a)) <= 5e-6 * max(abs(+1.47ae1eb853fc5X-00a), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 2
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.year#1.firm"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,2] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,3] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,3] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
matrix __stfx_S = e(stfixest_status)
assert rowsof(__stfx_S) == 2
assert colsof(__stfx_S) == 4
assert __stfx_S[1,2] == 3
assert __stfx_S[2,2] == 5
generate byte __stfx_actual_sample = e(sample)
assert __stfx_actual_sample == __stfx_expected_sample
assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1

// Sealed receipt case: dof_none
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear
generate byte __stfx_expected_sample = 0
generate double __stfx_expected_resid = .
replace __stfx_expected_sample = 1 if inlist(obs_id, 3, 4, 11, 12, 15, 16, 23, 24)
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 3
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 4
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 11
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 12
replace __stfx_expected_resid = +1.9999bfffffff3X-005 if obs_id == 15
replace __stfx_expected_resid = -1.9999bfffffff3X-005 if obs_id == 16
replace __stfx_expected_resid = -1.9999bffffff7fX-005 if obs_id == 23
replace __stfx_expected_resid = +1.9999bffffff7fX-005 if obs_id == 24
_stfixest_estimate_ols_test y c.x1, absorb(worker firm#year) dof(none) residual(__stfx_reference_resid)
assert e(N) == (+1.0000000000000X+003)
assert e(N_full) == (+1.8000000000000X+004)
assert e(N_hdfe) == (+1.0000000000000X+001)
assert e(df_a) == (+1.c000000000000X+002)
assert e(df_m) == (+0.0000000000000X-3ff)
assert e(df_r) == (+1.0000000000000X+000)
assert e(num_singletons) == (+1.0000000000000X+004)
assert e(sumweights) == (+1.0000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker firm#year"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"ols"' 
matrix __stfx_b = e(b)
assert rowsof(__stfx_b) == 1
assert colsof(__stfx_b) == 2
local __stfx_names : rowfullnames __stfx_b
assert `"`__stfx_names'"' == `"y1"' 
local __stfx_names : colfullnames __stfx_b
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_b[1,1] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_b[1,2] - (+1.b99999c000000X+002)) <= 5e-9 * max(abs(+1.b99999c000000X+002), 1)
matrix __stfx_V = e(V)
assert rowsof(__stfx_V) == 2
assert colsof(__stfx_V) == 2
local __stfx_names : rowfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
local __stfx_names : colfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_V[1,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[1,2] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,2] - (+1.47ae1eb853fc5X-009)) <= 5e-6 * max(abs(+1.47ae1eb853fc5X-009), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 2
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.firm#1.year"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,2] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,3] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
assert abs(__stfx_D[2,3] - (+1.8000000000000X+001)) <= 5e-9 * max(abs(+1.8000000000000X+001), 1)
assert abs(__stfx_D[2,4] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
assert abs(__stfx_D[2,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
matrix __stfx_S = e(stfixest_status)
assert rowsof(__stfx_S) == 2
assert colsof(__stfx_S) == 4
assert __stfx_S[1,2] == 3
assert __stfx_S[2,2] == 5
generate byte __stfx_actual_sample = e(sample)
assert __stfx_actual_sample == __stfx_expected_sample
assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1

// Sealed receipt case: dof_firstpair
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear
generate byte __stfx_expected_sample = 0
generate double __stfx_expected_resid = .
replace __stfx_expected_sample = 1 if inlist(obs_id, 3, 4, 11, 12, 15, 16, 23, 24)
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 3
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 4
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 11
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 12
replace __stfx_expected_resid = +1.9999bfffffff3X-005 if obs_id == 15
replace __stfx_expected_resid = -1.9999bfffffff3X-005 if obs_id == 16
replace __stfx_expected_resid = -1.9999bffffff7fX-005 if obs_id == 23
replace __stfx_expected_resid = +1.9999bffffff7fX-005 if obs_id == 24
_stfixest_estimate_ols_test y c.x1, absorb(worker firm#year) dof(firstpair) residual(__stfx_reference_resid)
assert e(N) == (+1.0000000000000X+003)
assert e(N_full) == (+1.8000000000000X+004)
assert e(N_hdfe) == (+1.0000000000000X+001)
assert e(df_a) == (+1.8000000000000X+002)
assert e(df_m) == (+0.0000000000000X-3ff)
assert e(df_r) == (+1.0000000000000X+001)
assert e(num_singletons) == (+1.0000000000000X+004)
assert e(sumweights) == (+1.0000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker firm#year"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"ols"' 
matrix __stfx_b = e(b)
assert rowsof(__stfx_b) == 1
assert colsof(__stfx_b) == 2
local __stfx_names : rowfullnames __stfx_b
assert `"`__stfx_names'"' == `"y1"' 
local __stfx_names : colfullnames __stfx_b
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_b[1,1] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_b[1,2] - (+1.b99999c000000X+002)) <= 5e-9 * max(abs(+1.b99999c000000X+002), 1)
matrix __stfx_V = e(V)
assert rowsof(__stfx_V) == 2
assert colsof(__stfx_V) == 2
local __stfx_names : rowfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
local __stfx_names : colfullnames __stfx_V
assert `"`__stfx_names'"' == `"o.x1 _cons"' 
assert abs(__stfx_V[1,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[1,2] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,1] - (+0.0000000000000X-3ff)) <= 5e-6 * max(abs(+0.0000000000000X-3ff), 1e-10)
assert abs(__stfx_V[2,2] - (+1.47ae1eb853fc5X-00a)) <= 5e-6 * max(abs(+1.47ae1eb853fc5X-00a), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 2
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.firm#1.year"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,2] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,3] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,3] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
matrix __stfx_S = e(stfixest_status)
assert rowsof(__stfx_S) == 2
assert colsof(__stfx_S) == 4
assert __stfx_S[1,2] == 3
assert __stfx_S[2,2] == 5
generate byte __stfx_actual_sample = e(sample)
assert __stfx_actual_sample == __stfx_expected_sample
assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1

display "END_FIXTURE|estimate_ols_categorical_interaction_remaining_iid_parity|rc=0"
