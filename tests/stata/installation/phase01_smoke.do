version 18.0
clear all
set more off

capture noisily stfixest, version
assert _rc == 0
capture noisily stfixest, doctor threads(32) verbose
assert _rc == 0
capture noisily stfixest, selftest threads(32)
assert _rc == 0

sysuse auto, clear
local before_n = _N
local before_k = c(k)
capture noisily stfeols price c.mpg i.foreign, absorb(rep78) vce(robust) threads(32)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'

capture noisily stfepois price c.mpg i.foreign, absorb(rep78) vce(robust) threads(32)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'

di as result "phase01_smoke.do: PASS"
