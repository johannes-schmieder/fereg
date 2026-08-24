version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* Frozen licensed comparator:
* benchmarks/reports/captured/darwin-arm64/stata18/ols/
* 20260824T161810Z-9f2494d, case intercept_and_slope. The projection also
* binds this receipt in benchmarks/manifests/ols_hidden_numeric_v1.json.
* The independent Rust explicit-dummy oracle
* separately establishes nuisance/model rank 22, finite-regressor rank 0,
* mathematical residual DoF 26, and RSS .04 on this exact twofold replication.
import delimited using `"`root'/tests/fixtures/ols/ols_intercept_slope_positive_dof.csv"', clear

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) residual(__frg_slope_resid)

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
assert !missing(__frg_slope_resid)

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "o.x1 _cons"
assert e(fereg_status)[1,2] == 3
assert e(fereg_status)[2,2] == 5
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

assert e(fereg_fe)[1,1] == 1
assert e(fereg_fe)[2,1] == 4
assert e(fereg_fe)[3,1] == 1
assert e(fereg_certificate)[1,2] == 1
assert e(fereg_certificate)[2,2] == 1
assert e(fereg_certificate)[3,2] == 1
assert e(fereg_certificate)[24,2] == 1
* The omitted x1 row makes the full reported 2x2 covariance singular.
assert e(fereg_certificate)[31,2] == 0
assert e(fereg_certificate)[31,2] == det(e(V))
assert e(fereg_certificate)[32,2] == 1

* Frozen dof(continuous) reference on the same positive-residual-DoF sample.
_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) dof(continuous)
assert e(N) == 48
assert e(df_a) == 27
assert e(df_r) == 21
assert e(rank) == 0
assert abs(e(b)[1,2] - +1.9eaaaaa000001X+002) <= 1e-7
assert abs(e(V)[2,2] - +1.4ce1a54d8a2d8X-00f) <= 1e-10
assert abs(e(rss) - +1.47ae1eb85404dX-005) <= 4e-8
assert abs(e(rmse) - +1.65873d22b2949X-005) <= 1e-9
assert e(dof_table)[1,1] == 8
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 8
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 8
assert e(dof_table)[2,2] == 0
assert e(dof_table)[2,3] == 8
assert e(dof_table)[2,4] == 1
assert e(dof_table)[2,5] == 0
assert e(dof_table)[3,1] == 12
assert e(dof_table)[3,2] == 1
assert e(dof_table)[3,3] == 11
assert e(dof_table)[3,4] == 1
assert e(dof_table)[3,5] == 0

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) vce(robust)
assert e(N) == 48
assert e(df_m) == 0
assert e(df_a) == 26
assert e(df_r) == 22
assert e(report_constant) == 1
assert `"`e(vce)'"' == "robust"
assert abs(e(b)[1,2] - +1.9eaaaaa000001X+002) <= 1e-7
assert abs(e(V)[2,2] - +1.3dc01dca03e5aX-00f) <= 1e-10

_fereg_ols_hidden_numeric_v1 y c.x1 [aw=aw], ///
    absorb(firm##c.time worker)
assert e(N) == 48
assert e(df_m) == 0
assert e(df_a) == 26
assert e(df_r) == 22
assert e(report_constant) == 1
assert abs(e(b)[1,2] - +1.c8419b5995a3cX+002) <= 1e-7
assert abs(e(V)[2,2] - +1.189d8f3836644X-00f) <= 1e-10

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) vce(cluster cluster_a)
assert e(N) == 48
assert e(df_m) == 0
assert e(df_a) == 19
assert e(df_r) == 7
assert e(report_constant) == 1
assert `"`e(vce)'"' == "cluster"
assert abs(e(b)[1,2] - +1.9eaaaaa000001X+002) <= 1e-7
assert abs(e(V)[2,2] - +1.da8326df3033eX-068) <= 1e-24

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) dof(none)
assert e(N) == 48
assert e(df_m) == 0
assert e(df_a) == 27
assert e(df_r) == 21
assert e(report_constant) == 1
assert abs(e(b)[1,2] - +1.9eaaaaa000001X+002) <= 1e-7
assert abs(e(V)[2,2] - +1.4ce1a54d8a2d8X-00f) <= 1e-10

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) dof(firstpair)
assert e(N) == 48
assert e(df_m) == 0
assert e(df_a) == 26
assert e(df_r) == 22
assert e(report_constant) == 1
assert abs(e(b)[1,2] - +1.9eaaaaa000001X+002) <= 1e-7
assert abs(e(V)[2,2] - +1.3dc01dca03e5aX-00f) <= 1e-10
capture noisily _fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm#c.time worker)
assert _rc == 0
capture noisily _fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker##c.x1)
assert _rc == 0

capture noisily fereg y c.x1, absorb(firm##c.time worker)
assert _rc == 0
assert `"`e(cmd)'"' == "fereg"
assert e(df_a) == 26
assert e(df_r) == 22

di as result "END_FIXTURE|estimate_ols_intercept_slope_parity|rc=0"
