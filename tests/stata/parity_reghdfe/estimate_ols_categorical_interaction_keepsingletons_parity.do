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
* case: target_keepsingletons
* payload_sha256: 1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955
* This fixture consumes the committed sealed receipt; it does not invoke reghdfe.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

generate double __frg_expected_resid = 0
replace __frg_expected_resid = -1.99997ffffffb6X-005 if obs_id == 3
replace __frg_expected_resid = +1.99997ffffffb6X-005 if obs_id == 4
replace __frg_expected_resid = +1.99997ffffffb6X-005 if obs_id == 11
replace __frg_expected_resid = -1.99997ffffffb6X-005 if obs_id == 12
replace __frg_expected_resid = +1.9999bffffffe7X-005 if obs_id == 15
replace __frg_expected_resid = -1.9999bffffffe7X-005 if obs_id == 16
replace __frg_expected_resid = -1.9999c00000047X-005 if obs_id == 23
replace __frg_expected_resid = +1.9999c00000047X-005 if obs_id == 24

_fereg_estimate_ols_test y c.x1, absorb(worker firm#year) ///
    keepsingletons residual(__frg_reference_resid)

assert e(N) == 24
assert e(N_full) == 24
assert e(num_singletons) == 0
assert e(drop_singletons) == 0
assert e(N_hdfe) == 2
assert e(df_m) == 0
assert e(df_a) == 22
assert e(df_r) == 2
assert e(sumweights) == 24
assert e(rank) == 0
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "worker firm#year"
assert `"`e(vce)'"' == "ols"

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "o.x1 _cons"
assert e(fereg_status)[1,2] == 3
assert e(fereg_status)[2,2] == 5
assert e(b)[1,1] == 0
assert e(V)[1,1] == 0
assert e(V)[1,2] == 0
assert e(V)[2,1] == 0
assert abs(e(b)[1,2] - +1.9eaaaaa000000X+002) <= 3e-6
assert abs(e(V)[2,2] - +1.b4e828f5c551eX-00c) <= 3e-6

assert rowsof(e(dof_table)) == 2
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.worker 1.firm#1.year"
assert e(dof_table)[1,1] == 12
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 12
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 20
assert e(dof_table)[2,2] == 10
assert e(dof_table)[2,3] == 10
assert e(dof_table)[2,4] == 0
assert e(dof_table)[2,5] == 0

assert e(sample) == 1
assert !missing(__frg_reference_resid)
assert abs(__frg_reference_resid - __frg_expected_resid) <= 3e-6

di as result "END_FIXTURE|estimate_ols_categorical_interaction_keepsingletons_parity|rc=0"
