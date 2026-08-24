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

// Sealed receipt case: nested_simple_cluster
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
_stfixest_estimate_ols_test y c.x1, absorb(worker firm#year) cluster(firm) residual(__stfx_reference_resid)
assert e(N) == (+1.0000000000000X+003)
assert e(N_clust) == (+1.0000000000000X+002)
assert e(N_clust1) == (+1.0000000000000X+002)
assert e(N_clustervars) == (+1.0000000000000X+000)
assert e(N_full) == (+1.8000000000000X+004)
assert e(N_hdfe) == (+1.0000000000000X+001)
assert e(df_a) == (+1.8000000000000X+001)
assert e(df_m) == (+0.0000000000000X-3ff)
assert e(df_r) == (+1.8000000000000X+001)
assert e(num_singletons) == (+1.0000000000000X+004)
assert e(sumweights) == (+1.0000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker firm#year"' 
local __stfx_macro `"`e(clustvar)'"' 
assert `"`__stfx_macro'"' == `"firm"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"cluster"' 
local __stfx_macro `"`e(vcetype)'"' 
assert `"`__stfx_macro'"' == `"Robust"' 
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
assert abs(__stfx_V[2,2] - (+1.eaaaabaaaaaffX-069)) <= 5e-6 * max(abs(+1.eaaaabaaaaaffX-069), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 2
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.firm#1.year"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,2] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
assert abs(__stfx_D[1,3] - (+1.8000000000000X+001)) <= 5e-9 * max(abs(+1.8000000000000X+001), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,3] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,5] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
matrix __stfx_S = e(stfixest_status)
assert rowsof(__stfx_S) == 2
assert colsof(__stfx_S) == 4
assert __stfx_S[1,2] == 3
assert __stfx_S[2,2] == 5
generate byte __stfx_actual_sample = e(sample)
assert __stfx_actual_sample == __stfx_expected_sample
assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1
assert e(stfixest_certificate)[1,1] == 1

// Sealed receipt case: identical_interaction_cluster
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
_stfixest_estimate_ols_test y c.x1, absorb(worker firm#year) cluster(firm#year) residual(__stfx_reference_resid)
assert e(N) == (+1.0000000000000X+003)
assert e(N_clust) == (+1.0000000000000X+002)
assert e(N_clust1) == (+1.0000000000000X+002)
assert e(N_clustervars) == (+1.0000000000000X+000)
assert e(N_full) == (+1.8000000000000X+004)
assert e(N_hdfe) == (+1.0000000000000X+001)
assert e(df_a) == (+1.8000000000000X+001)
assert e(df_m) == (+0.0000000000000X-3ff)
assert e(df_r) == (+1.8000000000000X+001)
assert e(num_singletons) == (+1.0000000000000X+004)
assert e(sumweights) == (+1.0000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker firm#year"' 
local __stfx_macro `"`e(clustvar)'"' 
assert `"`__stfx_macro'"' == `"firm#year"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"cluster"' 
local __stfx_macro `"`e(vcetype)'"' 
assert `"`__stfx_macro'"' == `"Robust"' 
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
assert abs(__stfx_V[2,2] - (+1.eaaaabaaaaaffX-069)) <= 5e-6 * max(abs(+1.eaaaabaaaaaffX-069), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 2
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.firm#1.year"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[1,2] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
assert abs(__stfx_D[1,3] - (+1.8000000000000X+001)) <= 5e-9 * max(abs(+1.8000000000000X+001), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+002)) <= 5e-9 * max(abs(+1.0000000000000X+002), 1)
assert abs(__stfx_D[2,3] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,5] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
matrix __stfx_S = e(stfixest_status)
assert rowsof(__stfx_S) == 2
assert colsof(__stfx_S) == 4
assert __stfx_S[1,2] == 3
assert __stfx_S[2,2] == 5
generate byte __stfx_actual_sample = e(sample)
assert __stfx_actual_sample == __stfx_expected_sample
assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1
assert e(stfixest_certificate)[1,1] == 1

// Sealed receipt case: unrelated_interaction_cluster
clear
set obs 12
generate long obs_id = 100 + _n
generate double y = real(word("3.20 4.05 4.60 5.25 5.90 6.35 7.10 7.85 8.10 8.75 9.30 10.05", _n))
generate double x = real(word("-1.40 -0.90 -0.45 0.05 0.40 0.85 1.15 1.65 -1.10 -0.25 0.70 1.90", _n))
generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 4 4", _n))
generate int firm = real(word("10 10 20 20 10 10 20 20 10 30 30 40", _n))
generate int year = real(word("2020 2020 2021 2021 2020 2020 2021 2021 2020 2022 2022 2023", _n))
generate int cluster_u = real(word("1 2 1 2 1 2 1 2 1 2 1 2", _n))
generate int cluster_v = real(word("1 2 2 1 2 1 1 2 1 1 2 2", _n))
generate byte __stfx_expected_sample = 0
generate double __stfx_expected_resid = .
replace __stfx_expected_sample = 1 if inlist(obs_id, 101, 102, 103, 104, 105, 106, 107, 108)
replace __stfx_expected_resid = -1.2e93f80ed7b3fX-006 if obs_id == 101
replace __stfx_expected_resid = +1.49660abdc3223X-003 if obs_id == 102
replace __stfx_expected_resid = -1.f264f7c3d9580X-005 if obs_id == 103
replace __stfx_expected_resid = -1.4df49b95e3ae6X-004 if obs_id == 104
replace __stfx_expected_resid = +1.70b7d698eadb6X-008 if obs_id == 105
replace __stfx_expected_resid = -1.2f194a70af820X-003 if obs_id == 106
replace __stfx_expected_resid = +1.024f9d922dc42X-005 if obs_id == 107
replace __stfx_expected_resid = +1.c5ff48aeb97b1X-004 if obs_id == 108
_stfixest_estimate_ols_test y c.x, absorb(worker firm#year) cluster(cluster_u#cluster_v) residual(__stfx_reference_resid)
assert e(N) == (+1.0000000000000X+003)
assert e(N_clust) == (+1.0000000000000X+002)
assert e(N_clust1) == (+1.0000000000000X+002)
assert e(N_clustervars) == (+1.0000000000000X+000)
assert e(N_full) == (+1.8000000000000X+003)
assert e(N_hdfe) == (+1.0000000000000X+001)
assert e(df_a) == (+1.8000000000000X+001)
assert e(df_m) == (+1.0000000000000X+000)
assert e(df_r) == (+1.8000000000000X+001)
assert e(num_singletons) == (+1.0000000000000X+002)
assert e(sumweights) == (+1.0000000000000X+003)
local __stfx_macro `"`e(absvars)'"' 
assert `"`__stfx_macro'"' == `"worker firm#year"' 
local __stfx_macro `"`e(clustvar)'"' 
assert `"`__stfx_macro'"' == `"cluster_u#cluster_v"' 
local __stfx_macro `"`e(vce)'"' 
assert `"`__stfx_macro'"' == `"cluster"' 
local __stfx_macro `"`e(vcetype)'"' 
assert `"`__stfx_macro'"' == `"Robust"' 
matrix __stfx_b = e(b)
assert rowsof(__stfx_b) == 1
assert colsof(__stfx_b) == 2
local __stfx_names : rowfullnames __stfx_b
assert `"`__stfx_names'"' == `"y1"' 
local __stfx_names : colfullnames __stfx_b
assert `"`__stfx_names'"' == `"x _cons"' 
assert abs(__stfx_b[1,1] - (+1.576510c34bad4X+000)) <= 5e-9 * max(abs(+1.576510c34bad4X+000), 1)
assert abs(__stfx_b[1,2] - (+1.53e9bc7e29351X+002)) <= 5e-9 * max(abs(+1.53e9bc7e29351X+002), 1)
matrix __stfx_V = e(V)
assert rowsof(__stfx_V) == 2
assert colsof(__stfx_V) == 2
local __stfx_names : rowfullnames __stfx_V
assert `"`__stfx_names'"' == `"x _cons"' 
local __stfx_names : colfullnames __stfx_V
assert `"`__stfx_names'"' == `"x _cons"' 
assert abs(__stfx_V[1,1] - (+1.22a49f7111152X-004)) <= 5e-6 * max(abs(+1.22a49f7111152X-004), 1e-10)
assert abs(__stfx_V[1,2] - (+1.5d495a9f749d3X-008)) <= 5e-6 * max(abs(+1.5d495a9f749d3X-008), 1e-10)
assert abs(__stfx_V[2,1] - (+1.5d495a9f749d3X-008)) <= 5e-6 * max(abs(+1.5d495a9f749d3X-008), 1e-10)
assert abs(__stfx_V[2,2] - (+1.dcef7be02984dX-00b)) <= 5e-6 * max(abs(+1.dcef7be02984dX-00b), 1e-10)
matrix __stfx_D = e(dof_table)
assert rowsof(__stfx_D) == 2
assert colsof(__stfx_D) == 5
local __stfx_names : rowfullnames __stfx_D
assert `"`__stfx_names'"' == `"1.worker 1.firm#1.year"' 
local __stfx_names : colfullnames __stfx_D
assert `"`__stfx_names'"' == `"Categories Redundant Num Coefs Inexact? Nested?"' 
assert abs(__stfx_D[1,1] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[1,2] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,3] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[1,4] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[1,5] - (+0.0000000000000X-3ff)) <= 5e-9 * max(abs(+0.0000000000000X-3ff), 1)
assert abs(__stfx_D[2,1] - (+1.0000000000000X+001)) <= 5e-9 * max(abs(+1.0000000000000X+001), 1)
assert abs(__stfx_D[2,2] - (+1.0000000000000X+000)) <= 5e-9 * max(abs(+1.0000000000000X+000), 1)
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
assert e(stfixest_certificate)[1,1] == 1

display "END_FIXTURE|estimate_ols_categorical_interaction_cluster_parity|rc=0"
