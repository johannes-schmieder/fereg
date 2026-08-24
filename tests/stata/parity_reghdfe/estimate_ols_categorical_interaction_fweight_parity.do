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
* case: fweight_frequency_cascade
* payload_sha256: 1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955
* This fixture consumes the committed sealed receipt; it does not invoke reghdfe.
clear
set obs 12
generate long obs_id = 100 + _n
generate double y = real(word("3.20 4.05 4.60 5.25 5.90 6.35 7.10 7.85 8.10 8.75 9.30 10.05", _n))
generate double x = real(word("-1.40 -0.90 -0.45 0.05 0.40 0.85 1.15 1.65 -1.10 -0.25 0.70 1.90", _n))
generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 4 4", _n))
generate int firm = real(word("10 10 20 20 10 10 20 20 10 30 30 40", _n))
generate int year = real(word("2020 2020 2021 2021 2020 2020 2021 2021 2020 2022 2022 2023", _n))
generate long fw = real(word("1 1 1 1 1 1 1 1 1 1 1 2", _n))

generate double __stfx_expected_resid = .
replace __stfx_expected_resid = -1.2e93f80ed79c5X-006 if obs_id == 101
replace __stfx_expected_resid = +1.49660abdc325bX-003 if obs_id == 102
replace __stfx_expected_resid = -1.f264f7c3d9567X-005 if obs_id == 103
replace __stfx_expected_resid = -1.4df49b95e3adaX-004 if obs_id == 104
replace __stfx_expected_resid = +1.70b7d698ea7ecX-008 if obs_id == 105
replace __stfx_expected_resid = -1.2f194a70af81fX-003 if obs_id == 106
replace __stfx_expected_resid = +1.024f9d922ddb0X-005 if obs_id == 107
replace __stfx_expected_resid = +1.c5ff48aeb9877X-004 if obs_id == 108
replace __stfx_expected_resid = +1.7be70c648845bX-032 if obs_id == 109
replace __stfx_expected_resid = +1.8d3021e9c959fX-033 if obs_id == 110
replace __stfx_expected_resid = -1.bf35d236b16dbX-033 if obs_id == 111
replace __stfx_expected_resid = +1.a37e7531f95dbX-032 if obs_id == 112

_stfixest_estimate_ols_test y c.x, absorb(worker firm#year) ///
    fweight(fw) residual(__stfx_reference_resid)

assert e(N) == 13
assert e(N_full) == 13
assert e(num_singletons) == 0
assert e(drop_singletons) == 1
assert e(N_hdfe) == 2
assert e(df_m) == 1
assert e(df_a) == 7
assert e(df_r) == 5
assert e(sumweights) == 13
assert e(rank) == 1
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "worker firm#year"
assert `"`e(vce)'"' == "ols"
assert `"`e(wtype)'"' == "fweight"
assert `"`e(wexp)'"' == "= fw"

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "x _cons"
assert e(stfixest_status)[1,2] == 1
assert e(stfixest_status)[2,2] == 5
assert abs(e(b)[1,1] - +1.576510c34bacfX+000) <= 3e-6
assert abs(e(b)[1,2] - +1.a01158a444c63X+002) <= 3e-6
assert abs(e(V)[1,1] - +1.de732871f9384X-006) <= 3e-6
assert abs(e(V)[1,2] - -1.4b3c084ee79d1X-007) <= 3e-6
assert abs(e(V)[2,1] - -1.4b3c084ee79d1X-007) <= 3e-6
assert abs(e(V)[2,2] - +1.2dae85efd719dX-008) <= 3e-6

assert rowsof(e(dof_table)) == 2
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.worker 1.firm#1.year"
assert e(dof_table)[1,1] == 4
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 4
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 4
assert e(dof_table)[2,2] == 1
assert e(dof_table)[2,3] == 3
assert e(dof_table)[2,4] == 0
assert e(dof_table)[2,5] == 0

assert e(sample) == 1
assert !missing(__stfx_reference_resid)
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 3e-6

di as result "END_FIXTURE|estimate_ols_categorical_interaction_fweight_parity|rc=0"
