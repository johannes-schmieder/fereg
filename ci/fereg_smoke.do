version 18.0
clear all
set more off
set varabbrev off

local legacy_prefix "st"
capture which `legacy_prefix'feols
assert _rc == 111
capture which `legacy_prefix'fepois
assert _rc == 111
capture which `legacy_prefix'fixest
assert _rc == 111

capture noisily fereg, version
assert _rc == 0
assert "`r(plugin_name)'" == "fereg_macos.plugin"

capture noisily fereg, selftest threads(4)
assert _rc == 0

sysuse auto, clear
local before_n = _N
local before_k = c(k)
capture noisily fereg price c.mpg i.foreign, absorb(rep78) vce(robust) threads(4)
assert _rc == 0
assert `"`e(cmd)'"' == "fereg"
assert e(N) > 0
assert _N == `before_n'
assert c(k) == `before_k'

display as result "FEREG_CI_OK"
display as result "END_FIXTURE|fereg_smoke|rc=0"
