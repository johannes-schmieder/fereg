version 18.0
clear all
set more off
local package `"$FEREG_STAGED_PACKAGE_ROOT"'
assert `"`package'"' != ""
local native "$FEREG_STAGED_PLUGIN_NAME"
set obs 60
generate double x = sin(_n)
generate double y = 2 + x/3 + cos(_n/5)/4
quietly fereg y x
matrix before = e(b)

* Mutate only this extracted test package, restoring each file before asserting.
* Loading the handle first also checks that cached handles cannot hide deletion.
quietly fereg, version
copy `"`package'/`native'"' `"`package'/`native'.saved"', replace
erase `"`package'/`native'"'
local failures = 0
foreach command in fereg feppml {
    capture noisily `command' y x
    if _rc != 601 local ++failures
    if mreldif(e(b), before) != 0 local ++failures
}
foreach action in version doctor selftest {
    capture noisily fereg, `action'
    if _rc != 601 local ++failures
}
copy `"`package'/`native'.saved"' `"`package'/`native'"', replace
erase `"`package'/`native'.saved"'
assert `failures' == 0
quietly fereg, selftest
di as result "END_FIXTURE|staged_plugin_binary|rc=0"
