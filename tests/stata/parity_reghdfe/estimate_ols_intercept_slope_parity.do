version 18.0
clear all
set more off

quietly findfile stfixest.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/stfixest.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* Frozen licensed comparator:
* benchmarks/reports/captured/darwin-arm64/stata18/ols/
* 20260821T030000Z-a5181f9, case intercept_and_slope.
* The independent Rust explicit-dummy oracle separately establishes numerical
* nuisance rank 22, positive mathematical residual DoF 2, and RSS .02.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

_stfixest_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) residual(__stfx_slope_resid)

assert e(N) == 24
assert e(N_full) == 24
assert e(df_m) == 0
assert e(df_a) == 26
assert e(df_r) == -2
assert e(N_hdfe) == 2
assert e(N_hdfe_extended) == 3
assert e(num_singletons) == 0
assert e(rank) == 0
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "firm##c.time worker"
assert `"`e(vce)'"' == "ols"
assert e(sample) == 1
assert !missing(__stfx_slope_resid)

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "o.x1 _cons"
assert e(stfixest_status)[1,2] == 3
assert e(stfixest_status)[2,2] == 5
assert e(b)[1,1] == 0
assert e(V)[1,1] == 0
assert e(V)[1,2] == 0
assert e(V)[2,1] == 0
assert abs(e(b)[1,2] - +1.9eaaaaa000000X+002) <= 1e-7
assert abs(e(V)[2,2] - -1.b4e828f5c5539X-00c) <= 1e-10
* import delimited retains the fixture outcome as float; the independent
* double-precision oracle's exact .02 therefore differs by 9.54e-9 here.
assert abs(e(rss) - 0.02) <= 2e-8

assert rowsof(e(dof_table)) == 3
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.firm 1.firm#c.time 1.worker"
assert e(dof_table)[1,1] == 8
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 8
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 8
assert e(dof_table)[2,2] == 0
assert e(dof_table)[2,3] == 8
assert e(dof_table)[2,4] == 0
assert e(dof_table)[2,5] == 0
assert e(dof_table)[3,1] == 12
assert e(dof_table)[3,2] == 2
assert e(dof_table)[3,3] == 10
assert e(dof_table)[3,4] == 0
assert e(dof_table)[3,5] == 0

assert e(stfixest_fe)[1,1] == 1
assert e(stfixest_fe)[2,1] == 4
assert e(stfixest_fe)[3,1] == 1
assert e(stfixest_certificate)[1,2] == 1
assert e(stfixest_certificate)[2,2] == 1
assert e(stfixest_certificate)[3,2] == 1
assert e(stfixest_certificate)[24,2] == 1
assert e(stfixest_certificate)[31,2] == 0
assert e(stfixest_certificate)[32,2] == 1

capture noisily _stfixest_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) vce(robust)
assert _rc == 498
capture noisily _stfixest_ols_hidden_numeric_v1 y c.x1 [aw=aw], ///
    absorb(firm##c.time worker)
assert _rc == 498
capture noisily _stfixest_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) dof(none)
assert _rc == 498
capture noisily _stfixest_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm#c.time worker)
assert _rc == 498
capture noisily _stfixest_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker##c.x1)
assert _rc == 498

capture noisily stfeols y c.x1, absorb(firm##c.time worker)
assert _rc == 498

di as result "END_FIXTURE|estimate_ols_intercept_slope_parity|rc=0"
