version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

* This historical disconnected 24-row design has explicit-dummy nuisance
* rank 22 and mathematical residual DoF 2, but the pinned conservative
* absorbed count is 26. fereg requires positive published residual DoF and
* therefore fails before covariance, statistics, or transaction publication.
quietly regress y c.x1 if obs_id <= 20
generate byte __frg_prior_sample = e(sample)
tempname prior_b prior_v
matrix `prior_b' = e(b)
matrix `prior_v' = e(V)
local prior_cmd `"`e(cmd)'"'
local prior_N = e(N)
local prior_df_r = e(df_r)

capture noisily _fereg_estimate_ols_test y c.x1, ///
    absorb(firm##c.time worker) residual(__frg_forbidden_resid)
assert _rc == 2001
confirm new variable __frg_forbidden_resid
assert `"`e(cmd)'"' == `"`prior_cmd'"'
assert e(N) == `prior_N'
assert e(df_r) == `prior_df_r'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_v') == 0
assert e(sample) == __frg_prior_sample
capture confirm matrix e(fereg_receipt)
assert _rc == 111

ereturn clear
capture noisily _fereg_estimate_ols_test y c.x1, ///
    absorb(firm##c.time worker) residual(__frg_null_forbidden_resid)
assert _rc == 2001
confirm new variable __frg_null_forbidden_resid
local null_scalars : e(scalars)
local null_macros : e(macros)
local null_matrices : e(matrices)
local null_functions : e(functions)
assert `"`null_scalars'`null_macros'`null_matrices'`null_functions'"' == ""

capture noisily fereg y c.x1, absorb(firm##c.time worker)
assert _rc == 498

di as result "END_FIXTURE|estimate_ols_intercept_slope_overabsorbed_fail_closed|rc=0"
