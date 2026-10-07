version 19.0
clear all
set more off
set varabbrev off
args compare_reghdfe

* The optional comparator requires the isolated references.lock.toml runtime:
* reghdfe 6.14.1, commit 29b2b203f534413369d675d0c0f674d61291f536.
* This fixture qualifies reference behavior; integration is M5.
local estimators regress
if "`compare_reghdfe'" == "1" local estimators regress reghdfe
foreach estimator of local estimators {
    foreach bad in -1 1.5 {
        foreach scenario in eligible missing_y missing_x excluded_if excluded_in zero_weight missing_weight {
            clear
            quietly set obs 40
            generate double y = 1 + mod(_n, 7) + _n/10
            generate double x = _n
            generate double g = mod(_n, 3)
            generate long firm = mod(_n, 4)
            generate double w = 1
            quietly replace g = `bad' in 1
            if "`scenario'" == "missing_y" quietly replace y = . in 1
            if "`scenario'" == "missing_x" quietly replace x = . in 1
            if "`scenario'" == "zero_weight" quietly replace w = 0 in 1
            if "`scenario'" == "missing_weight" quietly replace w = . in 1
            local restriction
            if "`scenario'" == "excluded_if" local restriction "if _n > 1"
            if "`scenario'" == "excluded_in" local restriction "in 2/l"
            local weight
            if inlist("`scenario'", "zero_weight", "missing_weight") local weight "[aw=w]"
            if "`estimator'" == "regress" capture noisily regress y i.g x i.firm `restriction' `weight'
            else capture noisily reghdfe y i.g x `restriction' `weight', absorb(firm)
            local result = _rc
            if "`scenario'" == "eligible" assert `result' == 452
            else if "`estimator'" == "reghdfe" assert `result' == 3598
            else {
                assert `result' == 0
                assert e(N) == 39
                assert e(sample) == (_n > 1)
            }
            display "FACTOR_PRECEDENCE|`estimator'|`bad'|`scenario'|`result'"
        }
    }
}
display "END_FIXTURE|reference_factor_precedence|rc=0"
