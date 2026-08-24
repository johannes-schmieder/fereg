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
* 20260824T135913Z-830fad8, case intercept_and_slope. The projection also
* binds this receipt in benchmarks/manifests/ols_hidden_numeric_v1.json.
* The independent Rust explicit-dummy oracle
* separately establishes nuisance/model rank 22, finite-regressor rank 0,
* mathematical residual DoF 26, and RSS .04 on this exact twofold replication.
import delimited using `"`root'/tests/fixtures/ols/ols_intercept_slope_positive_dof.csv"', clear

_stfixest_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) residual(__stfx_slope_resid)

assert e(N) == 48
assert e(N_full) == 48
assert e(df_m) == 0
assert e(df_a) == 26
assert e(df_r) == 22
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
assert abs(e(b)[1,2] - +1.9eaaaaa000001X+002) <= 1e-7
assert abs(e(V)[2,2] - +1.3dc01dca03e5aX-00f) <= 1e-10
assert e(V)[2,2] > 0
assert e(rmse) > 0
assert abs(e(rss) - 0.04) <= 4e-8

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
* reghdfe marks the conventional one-slope-per-category count inexact.
assert e(dof_table)[2,4] == 1
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
* The omitted x1 row makes the full reported 2x2 covariance singular.
assert e(stfixest_certificate)[31,2] == 0
assert e(stfixest_certificate)[31,2] == det(e(V))
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
