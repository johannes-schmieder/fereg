version 18.0
clear all
set more off

local legacy_prefix "st"
capture which `legacy_prefix'feols
assert _rc == 111
capture which `legacy_prefix'fepois
assert _rc == 111
capture which `legacy_prefix'fixest
assert _rc == 111

capture noisily fereg, version
local version_rc = _rc
local plugin_name "`r(plugin_name)'"
if `version_rc' != 0 {
    exit `version_rc'
}
if c(os) == "MacOSX" | (c(os) == "Unix" & strmatch(c(machine_type), "Mac*")) {
    if "`plugin_name'" != "fereg_macos.plugin" {
        di as error "wrong plugin selected for macOS: `plugin_name'"
        exit 9
    }
}
else if c(os) == "Windows" {
    if "`plugin_name'" != "fereg_windows.plugin" {
        di as error "wrong plugin selected for Windows: `plugin_name'"
        exit 9
    }
}
else if c(os) == "Unix" {
    if "`plugin_name'" != "fereg_linux.plugin" {
        di as error "wrong plugin selected for Linux: `plugin_name'"
        exit 9
    }
}
capture noisily fereg, doctor threads(32) verbose
local doctor_rc = _rc
if `doctor_rc' != 0 {
    exit `doctor_rc'
}
capture noisily fereg, selftest threads(32)
local selftest_rc = _rc
if `selftest_rc' != 0 {
    exit `selftest_rc'
}

sysuse auto, clear
local before_n = _N
local before_k = c(k)
capture noisily fereg price c.mpg i.foreign, absorb(rep78) vce(robust) threads(32)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'

capture noisily feppml price c.mpg i.foreign, absorb(rep78) vce(robust) threads(32)
assert _rc == 498
assert _N == `before_n'
assert c(k) == `before_k'

di as result "phase01_smoke.do: PASS"
di as result "END_FIXTURE|phase01_smoke|rc=0"
