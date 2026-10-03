version 18.0
clear all
set more off
local package `"$FEREG_STAGED_PACKAGE_ROOT"'
assert `"`package'"' != ""
local native "$FEREG_STAGED_PLUGIN_NAME"
local frozen = subinstr("`native'", "fereg_", "fereg_legacy_", 1)
set obs 60
generate double x = sin(_n)
generate double y = 2 + x/3 + cos(_n/5)/4
quietly fereg y x
matrix before = e(b)

* Mutate only this extracted test package, restoring each file before asserting.
* Loading the handle first also checks that cached handles cannot hide deletion.
foreach engine in next legacy {
    quietly fereg, version engine(`engine')
    local selected = cond("`engine'" == "next", "`native'", "`frozen'")
    copy `"`package'/`selected'"' `"`package'/`selected'.saved"', replace
    erase `"`package'/`selected'"'
    local failures = 0
    foreach command in fereg feppml {
        capture noisily `command' y x, engine(`engine')
        if _rc != 601 local ++failures
        if mreldif(e(b), before) != 0 local ++failures
        global FEREG_ENGINE `engine'
        capture noisily `command' y x
        if _rc != 601 local ++failures
        macro drop FEREG_ENGINE
        if "`engine'" == "next" {
            capture noisily `command' y x
            if _rc != 601 local ++failures
        }
    }
    foreach action in version doctor selftest {
        capture noisily fereg, `action' engine(`engine')
        if _rc != 601 local ++failures
    }
    copy `"`package'/`selected'.saved"' `"`package'/`selected'"', replace
    erase `"`package'/`selected'.saved"'
    assert `failures' == 0
    quietly fereg, selftest engine(`engine')
}
di as result "END_FIXTURE|staged_engine_binaries|rc=0"
