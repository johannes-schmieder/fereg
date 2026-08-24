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
* case: missing_interaction_components
* payload_sha256: 1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955
* This fixture consumes the committed sealed receipt; it does not invoke reghdfe.
clear
set obs 12
generate long obs_id = 200 + _n
generate double y = real(word("3.10 3.90 4.40 5.20 5.80 6.50 7.10 7.90 8.40 9.00 . 9.80", _n))
generate double x = real(word("-1.20 -0.70 -0.20 0.30 0.80 1.30 1.80 2.30 -1.00 -0.50 0.00 0.50", _n))
generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 3 3", _n))
generate int firm = real(word("10 10 20 20 10 10 20 20 . 30 30 30", _n))
generate int year = real(word("2020 2020 2021 2021 2020 2020 2021 2021 2020 . 2022 2022", _n))

generate byte __stfx_expected_sample = inrange(obs_id, 201, 208) | obs_id == 212
generate double __stfx_expected_resid = .
replace __stfx_expected_resid = +1.2f98fe2d949feX-033 if obs_id == 201
replace __stfx_expected_resid = +1.9999999999938X-006 if obs_id == 202
replace __stfx_expected_resid = -1.9999999999928X-006 if obs_id == 203
replace __stfx_expected_resid = -1.2f98fe2d949feX-033 if obs_id == 204
replace __stfx_expected_resid = +1.9999999999928X-006 if obs_id == 205
replace __stfx_expected_resid = -1.99999999999e9X-005 if obs_id == 206
replace __stfx_expected_resid = -1.d843527fcabfcX-033 if obs_id == 207
replace __stfx_expected_resid = +1.9999999999a14X-006 if obs_id == 208
replace __stfx_expected_resid = +0.0000000000000X-3ff if obs_id == 212

_stfixest_estimate_ols_test y c.x, absorb(worker firm#year) ///
    keepsingletons residual(__stfx_reference_resid)

assert e(N) == 9
assert e(N_full) == 9
assert e(num_singletons) == 0
assert e(drop_singletons) == 0
assert e(N_hdfe) == 2
assert e(df_m) == 1
assert e(df_a) == 4
assert e(df_r) == 4
assert e(sumweights) == 9
assert e(rank) == 1
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "worker firm#year"
assert `"`e(vce)'"' == "ols"

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "x _cons"
assert e(stfixest_status)[1,2] == 1
assert e(stfixest_status)[2,2] == 5
assert abs(e(b)[1,1] - +1.8ccccccccccd1X+000) <= 3e-6
assert abs(e(b)[1,2] - +1.47db97530eca8X+002) <= 3e-6
assert abs(e(V)[1,1] - +1.47ae147ae1493X-009) <= 3e-6
assert abs(e(V)[1,2] - -1.64ce9ed572779X-00a) <= 3e-6
assert abs(e(V)[2,1] - -1.64ce9ed572779X-00a) <= 3e-6
assert abs(e(V)[2,2] - +1.cd5750cad6873X-00b) <= 3e-6

assert rowsof(e(dof_table)) == 2
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.worker 1.firm#1.year"
assert e(dof_table)[1,1] == 3
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 3
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 3
assert e(dof_table)[2,2] == 1
assert e(dof_table)[2,3] == 2
assert e(dof_table)[2,4] == 0
assert e(dof_table)[2,5] == 0

assert e(sample) == __stfx_expected_sample
assert missing(__stfx_reference_resid) if !__stfx_expected_sample
assert !missing(__stfx_reference_resid) if __stfx_expected_sample
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 3e-6 if __stfx_expected_sample

di as result "END_FIXTURE|estimate_ols_categorical_interaction_missing_parity|rc=0"
