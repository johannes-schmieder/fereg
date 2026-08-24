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
* case: repeated_identical_interaction
* payload_sha256: 1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955
* This fixture consumes the committed sealed receipt; it does not invoke reghdfe.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

generate byte __stfx_expected_sample = inlist(obs_id, 3, 4, 11, 12, 15, 16, 23, 24)
generate double __stfx_expected_resid = .
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 3
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 4
replace __stfx_expected_resid = +1.99997fffffffdX-005 if obs_id == 11
replace __stfx_expected_resid = -1.99997fffffffdX-005 if obs_id == 12
replace __stfx_expected_resid = +1.9999bfffffff3X-005 if obs_id == 15
replace __stfx_expected_resid = -1.9999bfffffff3X-005 if obs_id == 16
replace __stfx_expected_resid = -1.9999bffffff7fX-005 if obs_id == 23
replace __stfx_expected_resid = +1.9999bffffff7fX-005 if obs_id == 24

_stfixest_estimate_ols_test y c.x1, absorb(worker firm#year firm#year) ///
    residual(__stfx_reference_resid)

assert e(N) == 8
assert e(N_full) == 24
assert e(num_singletons) == 16
assert e(drop_singletons) == 1
assert e(N_hdfe) == 2
assert e(df_m) == 0
assert e(df_a) == 6
assert e(df_r) == 2
assert e(sumweights) == 8
assert e(rank) == 0
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "worker firm#year firm#year"
assert `"`e(vce)'"' == "ols"

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "o.x1 _cons"
assert e(stfixest_status)[1,2] == 3
assert e(stfixest_status)[2,2] == 5
assert e(b)[1,1] == 0
assert e(V)[1,1] == 0
assert e(V)[1,2] == 0
assert e(V)[2,1] == 0
assert abs(e(b)[1,2] - +1.b99999c000000X+002) <= 3e-6
assert abs(e(V)[2,2] - +1.47ae1eb853fc5X-00a) <= 3e-6

assert rowsof(e(dof_table)) == 3
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.worker 1.firm#1.year 1.firm#1.year"
assert e(dof_table)[1,1] == 4
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 4
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 4
assert e(dof_table)[2,2] == 2
assert e(dof_table)[2,3] == 2
assert e(dof_table)[2,4] == 0
assert e(dof_table)[2,5] == 0
assert e(dof_table)[3,1] == 4
assert e(dof_table)[3,2] == 4
assert e(dof_table)[3,3] == 0
assert e(dof_table)[3,4] == 1
assert e(dof_table)[3,5] == 0

assert e(sample) == __stfx_expected_sample
assert missing(__stfx_reference_resid) if !__stfx_expected_sample
assert abs(__stfx_reference_resid - __stfx_expected_resid) <= 3e-6 if __stfx_expected_sample

di as result "END_FIXTURE|estimate_ols_categorical_interaction_repeated_parity|rc=0"
