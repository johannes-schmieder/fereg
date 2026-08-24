version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* Positive-residual-DoF fixture shared with the independent Rust
* explicit-dummy oracle in intercept_slope_oracle.rs. Exact reghdfe parity is
* bound through the slope_only_multiple case in the sealed OLS receipt.
import delimited using `"`root'/tests/fixtures/ols/ols_slope_only_multiple_positive_dof.csv"', clear

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm#c.time worker#c.(time x2)) residuals(__frg_slope_only_resid)

assert e(N) == 60
assert e(N_full) == 60
assert e(df_m) == 1
assert e(df_a) == 17
assert e(df_r) == 41
assert e(N_hdfe) == 2
assert e(N_hdfe_extended) == 3
assert e(num_singletons) == 0
assert e(rank) == 1
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "firm#c.time worker#c.(time x2)"
assert `"`e(vce)'"' == "ols"
assert e(sample) == 1
assert !missing(__frg_slope_only_resid)

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "x1 _cons"
assert e(fereg_status)[1,2] == 1
assert e(fereg_status)[2,2] == 1
assert abs(e(b)[1,1] - .7) <= .02
assert abs(e(b)[1,2] - 1.5) <= .05
assert e(V)[1,1] > 0
assert e(V)[2,2] > 0
assert e(rss) > 0
assert e(rmse) > 0

assert rowsof(e(dof_table)) == 3
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.firm#c.time 1.worker#c.time 1.worker#c.x2"
assert e(dof_table)[1,1] == 5
assert e(dof_table)[2,1] == 6
assert e(dof_table)[3,1] == 6
forvalues row = 1/3 {
    assert e(dof_table)[`row',2] == 0
    assert e(dof_table)[`row',3] == e(dof_table)[`row',1]
    assert e(dof_table)[`row',4] == 1
    assert e(dof_table)[`row',5] == 0
    assert e(fereg_fe)[`row',1] == 4
}
assert e(fereg_certificate)[1,2] == 1
assert e(fereg_certificate)[2,2] == 1
assert e(fereg_certificate)[3,2] == 1
assert e(fereg_certificate)[24,2] == 1
assert e(fereg_certificate)[32,2] == 1

capture noisily _fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm#c.time worker#c.(time x2)) vce(robust)
assert _rc == 498
capture noisily _fereg_ols_hidden_numeric_v1 y c.x1 [aw=aw], ///
    absorb(firm#c.time worker#c.(time x2))
assert _rc == 498
capture noisily _fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm#c.time worker#c.(time x2)) dof(none)
assert _rc == 498

capture noisily fereg y c.x1, absorb(firm#c.time worker#c.(time x2))
assert _rc == 0
assert `"`e(cmd)'"' == "fereg"
assert e(N) == 60

di as result "END_FIXTURE|estimate_ols_slope_only_multiple_parity|rc=0"
