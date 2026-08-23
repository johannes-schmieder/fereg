version 18.0
clear all
set more off

quietly findfile stfixest.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/stfixest.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

set obs 72
generate long obs_id = _n
generate int firm = mod(_n - 1, 6) + 1
generate int year = mod(floor((_n - 1) / 6), 4) + 1
generate int repetition = floor((_n - 1) / 24)
generate double x = sin(_n / 5) + .31 * repetition + .07 * firm - .04 * year
generate double z = cos(_n / 7) - .22 * repetition + .03 * firm
generate double cell = .8 * firm - .45 * year + .11 * firm * year
generate double y = 1.6 * x - .7 * z + cell + .03 * sin(_n / 3)
generate int cluster_a = mod(firm + 2 * year + repetition, 9) + 1
generate int cluster_b = mod(2 * firm + year + 3 * repetition, 7) + 1

quietly regress y c.x c.z i.firm#i.year
local reference_x = _b[x]
local reference_z = _b[z]
local reference_vx = e(V)[1,1]
local reference_vz = e(V)[2,2]

_stfixest_estimate_ols_test y c.x c.z, absorb(firm#year) ///
    residual(__stfx_interaction_resid)
assert `"`e(cmd)'"' == "_stfixest_estimate_ols_test"
assert e(stfixest_receipt)[1,16] == 1
assert e(stfixest_receipt)[1,4] == 1
assert e(stfixest_receipt)[1,11] == 1
assert e(stfixest_fe)[1,1] == 3
assert e(stfixest_fe)[1,2] == 1
assert e(stfixest_fe)[1,3] == 0
assert e(stfixest_fe)[1,4] == 24
assert e(stfixest_fe)[1,6] == e(df_a)
assert e(sample) == 1
assert !missing(__stfx_interaction_resid)
assert reldif(_b[x], `reference_x') < 1e-8
assert reldif(_b[z], `reference_z') < 1e-8
assert reldif(e(V)[1,1], `reference_vx') < 1e-7
assert reldif(e(V)[2,2], `reference_vz') < 1e-7
assert e(stfixest_certificate)[32,2] == 1

_stfixest_estimate_ols_test y c.x c.z, absorb(firm#year) ///
    vce(robust) noconstant
assert e(stfixest_receipt)[1,4] == 2
assert e(stfixest_receipt)[1,5] == 2
assert e(stfixest_fe)[1,1] == 3
assert e(stfixest_certificate)[24,2] == 1
assert e(stfixest_certificate)[32,2] == 1

_stfixest_estimate_ols_test y c.x c.z, absorb(firm#year) ///
    cluster(cluster_a cluster_b)
assert e(stfixest_receipt)[1,4] == 3
assert e(stfixest_receipt)[1,10] == 2
assert e(stfixest_cluster)[1,3] > 1
assert e(stfixest_cluster)[2,3] > 1
assert e(stfixest_subset)[1,1] == 1
assert e(stfixest_subset)[2,1] == 2
assert e(stfixest_subset)[3,1] == 3
assert e(stfixest_certificate)[24,2] == 1
assert e(stfixest_certificate)[32,2] == 1
assert e(N_hdfe) == 1
assert `"`e(absvars)'"' == "firm#year"

* Frozen licensed-reghdfe case `multi_fe_interactions` from the sealed OLS-v1
* receipt. This jointly exercises an ordinary factor interaction, an absorbed
* categorical tuple, singleton deletion, post-absorption collinearity, the
* reported normalized constant, covariance, and two-row absorbed DoF.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear
_stfixest_estimate_ols_test y c.x1 i.industry##i.year, ///
    absorb(worker firm#year)
assert e(N) == 8
assert e(df_m) == 1
assert e(df_a) == 6
assert e(df_r) == 1
assert e(N_hdfe) == 2
assert `"`e(absvars)'"' == "worker firm#year"
assert e(stfixest_fe)[1,1] == 1
assert e(stfixest_fe)[2,1] == 3
assert e(stfixest_fe)[2,3] == 0
assert e(stfixest_fe)[1,4] == 4
assert e(stfixest_fe)[2,4] == 4
assert e(stfixest_fe)[1,6] + e(stfixest_fe)[2,6] == e(df_a)
assert colsof(e(b)) == 10
assert rowsof(e(dof_table)) == 2
local interaction_names : colfullnames e(b)
assert `"`interaction_names'"' == ///
    "o.x1 2b.industry 3o.industry 2021b.year 2022o.year 2b.industry#2021b.year 2b.industry#2022o.year 3o.industry#2021b.year 3.industry#2022.year _cons"
local interaction_dof_rows : rowfullnames e(dof_table)
assert `"`interaction_dof_rows'"' == "1.worker 1.firm#1.year"
forvalues column = 1/8 {
    assert e(b)[1,`column'] == 0
}
assert abs(e(b)[1,9] - +1.ffffffdcf4f1eX-017) <= 3e-6
assert abs(e(b)[1,10] - +1.b999998000000X+002) <= 3e-6
assert abs(e(V)[9,9] - +1.47ae1eb851fc4X-005) <= 3e-6
assert abs(e(V)[9,10] - -1.47ae1eb851fc4X-007) <= 3e-6
assert abs(e(V)[10,9] - -1.47ae1eb851fc4X-007) <= 3e-6
assert abs(e(V)[10,10] - +1.47ae1eb851fc5X-008) <= 3e-6
assert e(stfixest_certificate)[24,2] == 1
assert e(stfixest_certificate)[32,2] == 1

di as result "END_FIXTURE|estimate_ols_categorical_interaction|rc=0"
