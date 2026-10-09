version 18.0
clear all
set more off

if c(os) != "Windows" {
    exit 9
}

capture noisily fereg, version
local version_rc = _rc
local plugin_name `"`r(plugin_name)'"'
if `version_rc' {
    exit `version_rc'
}
assert `"`plugin_name'"' == "fereg_windows.plugin"

capture noisily fereg, doctor threads(2)
local doctor_rc = _rc
if `doctor_rc' {
    exit `doctor_rc'
}
capture noisily fereg, selftest threads(2)
local selftest_rc = _rc
if `selftest_rc' {
    exit `selftest_rc'
}

set obs 48
generate long obs_id = _n
generate byte fe = 1 + mod(_n - 1, 8)
generate double x = mod(7 * _n, 19) / 5 - 2
generate double y = 1 + 0.4 * x + 0.15 * fe + mod(_n, 3) / 10

fereg y c.x, absorb(fe) vce(robust) threads(2) deterministic
assert `"`e(cmd)'"' == "fereg"
assert e(N) == 48
assert e(sample) == 1
assert e(rank) >= 1
assert e(V)[1,1] >= 0 & e(V)[1,1] < .
assert _b[x] < .

do "tests/stata/installation/observation_stores.do"

display as result "END_FIXTURE|windows_ci_smoke|rc=0"
