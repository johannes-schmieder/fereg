version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* Accepted source-bound oracle:
* .ci/reference-probes/fe-interaction/e81b942c0dc0b122258489a60c956cdbe0121512.json
* case: tuple_second_fe_cascade
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

generate byte __frg_expected_sample = obs_id <= 108
generate double __frg_expected_resid = .
replace __frg_expected_resid = -1.2e93f80ed7b3fX-006 if obs_id == 101
replace __frg_expected_resid = +1.49660abdc3223X-003 if obs_id == 102
replace __frg_expected_resid = -1.f264f7c3d9580X-005 if obs_id == 103
replace __frg_expected_resid = -1.4df49b95e3ae6X-004 if obs_id == 104
replace __frg_expected_resid = +1.70b7d698eadb6X-008 if obs_id == 105
replace __frg_expected_resid = -1.2f194a70af820X-003 if obs_id == 106
replace __frg_expected_resid = +1.024f9d922dc42X-005 if obs_id == 107
replace __frg_expected_resid = +1.c5ff48aeb97b1X-004 if obs_id == 108

_fereg_estimate_ols_test y c.x, absorb(worker firm#year) residual(__frg_reference_resid)

assert e(N) == 8
assert e(N_full) == 12
assert e(num_singletons) == 4
assert e(drop_singletons) == 1
assert e(N_hdfe) == 2
assert e(df_m) == 1
assert e(df_a) == 3
assert e(df_r) == 4
assert e(sumweights) == 8
assert e(rank) == 1
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "worker firm#year"
assert `"`e(vce)'"' == "ols"

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "x _cons"
assert e(fereg_status)[1,2] == 1
assert e(fereg_status)[2,2] == 5
assert abs(e(b)[1,1] - +1.576510c34bad4X+000) <= 3e-6
assert abs(e(b)[1,2] - +1.53e9bc7e29351X+002) <= 3e-6
assert abs(e(V)[1,1] - +1.2b07f9473bbddX-005) <= 3e-6
assert abs(e(V)[1,2] - -1.93b12a202a404X-008) <= 3e-6
assert abs(e(V)[2,1] - -1.93b12a202a404X-008) <= 3e-6
assert abs(e(V)[2,2] - +1.ae3b26f81645dX-009) <= 3e-6

assert rowsof(e(dof_table)) == 2
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.worker 1.firm#1.year"
assert e(dof_table)[1,1] == 2
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 2
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 2
assert e(dof_table)[2,2] == 1
assert e(dof_table)[2,3] == 1
assert e(dof_table)[2,4] == 0
assert e(dof_table)[2,5] == 0

assert e(sample) == __frg_expected_sample
assert missing(__frg_reference_resid) if !__frg_expected_sample
assert abs(__frg_reference_resid - __frg_expected_resid) <= 3e-6 if __frg_expected_sample

di as result "END_FIXTURE|estimate_ols_categorical_interaction_cascade_parity|rc=0"
