version 18.0
clear all
set more off
set varabbrev off

capture noisily stfixest, version
assert _rc == 0
assert "`r(plugin_name)'" == "stfixest_macos.plugin"

capture noisily stfixest, selftest threads(4)
assert _rc == 0

sysuse auto, clear
local before_n = _N
local before_k = c(k)
capture noisily stfeols price c.mpg i.foreign, absorb(rep78) vce(robust) threads(4)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'

display as result "STFIXEST_CI_OK"
display as result "END_FIXTURE|stfixest_smoke|rc=0"
