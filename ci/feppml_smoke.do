version 18.0
clear all
set more off
set varabbrev off

set obs 120
generate long obs_id = _n
generate byte fe = 1 + mod(_n, 8)
generate double x = mod(7 * _n, 31) / 10 - 1.5
generate double y = 1 + mod(11 * _n, 17)
generate long before_order = _n

local before_n = _N
local before_k = c(k)
capture noisily feppml y c.x, absorb(fe) separation(none) ///
    vce(robust) threads(4) keepsingletons
assert _rc == 0
assert `"`e(cmd)'"' == "feppml"
assert e(converged) == 1
assert e(N) > 0
assert _b[x] < .
assert _se[x] < .
assert _N == `before_n'
assert c(k) == `before_k'
assert before_order == _n
count if e(sample)
assert r(N) == e(N)

display as result "FEREG_PPML_SMOKE_OK"
display as result "END_FIXTURE|feppml_smoke|rc=0"
