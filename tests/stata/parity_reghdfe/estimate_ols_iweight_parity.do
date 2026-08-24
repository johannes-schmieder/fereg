version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

import delimited using `"`root'/tests/fixtures/ols/ols_weights.csv"', clear
_fereg_ols_hidden_numeric_v1 y c.x, absorb(group)
tempname prior_b prior_v
matrix `prior_b' = e(b)
matrix `prior_v' = e(V)
local prior_cmd `"`e(cmd)'"'

capture noisily _fereg_ols_hidden_numeric_v1 y c.x [iw=iw_probe], absorb(group)
assert _rc == 101
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_v') == 0
assert `"`e(cmd)'"' == `"`prior_cmd'"'

capture noisily fereg y c.x [iw=iw_probe], absorb(group)
assert _rc == 101
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_v') == 0

di as result "END_FIXTURE|estimate_ols_iweight_parity|rc=0"
