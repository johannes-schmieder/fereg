version 18.0
clear all
set more off

set obs 24
generate long obs_id = _n
generate double x = mod(_n, 7) - 3
generate double y = 2 + 0.4 * x + 0.1 * mod(_n, 5)
generate int firm = mod(_n, 4) + 1
generate double pweight = 1 + mod(_n, 3)
generate double offset = 0.01 * mod(_n, 4)
generate double exposure = 1 + mod(_n, 5)
generate int cluster = mod(_n, 6) + 1

quietly regress y x
tempname before_b before_v
matrix `before_b' = e(b)
matrix `before_v' = e(V)
local before_cmd `"`e(cmd)'"'
local before_cmdline `"`e(cmdline)'"'
local before_n = _N
local before_k = c(k)
quietly datasignature
local before_signature `"`r(datasignature)'"'

capture noisily feppml y x [pw=pweight], absorb(firm) vce(cluster cluster) ///
    offset(offset) separation(fe simplex) residuals(__ppml_resid) ///
    savefes(__ppml_fe) threads(2) memory(low) solver(map)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'
confirm new variable __ppml_resid
confirm new variable __ppml_fe
assert `"`e(cmd)'"' == `"`before_cmd'"'
assert `"`e(cmdline)'"' == `"`before_cmdline'"'
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_v') == 0
quietly datasignature
assert `"`r(datasignature)'"' == `"`before_signature'"'

capture noisily feppml y x, exposure(exposure) iterate(3) tolerance(1e-6)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'
assert `"`e(cmd)'"' == `"`before_cmd'"'
assert `"`e(cmdline)'"' == `"`before_cmdline'"'
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_v') == 0
quietly datasignature
assert `"`r(datasignature)'"' == `"`before_signature'"'

capture noisily feppml y x, offset(offset) exposure(exposure)
assert _rc == 198
assert _N == `before_n'
assert c(k) == `before_k'
assert `"`e(cmd)'"' == `"`before_cmd'"'
assert `"`e(cmdline)'"' == `"`before_cmdline'"'
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_v') == 0
quietly datasignature
assert `"`r(datasignature)'"' == `"`before_signature'"'

capture noisily feppml y x, threads(0)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'
assert `"`e(cmd)'"' == `"`before_cmd'"'
assert `"`e(cmdline)'"' == `"`before_cmdline'"'
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_v') == 0
quietly datasignature
assert `"`r(datasignature)'"' == `"`before_signature'"'

display as text "END_FIXTURE|feppml_fail_closed|rc=0"
