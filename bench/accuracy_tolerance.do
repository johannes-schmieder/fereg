* Coefficient accuracy against tolerance(): descriptive and untimed.
* Fits one linear model at several tolerances, the first a 1e-14 reference,
* and posts every coefficient and standard error; a failed fit posts its
* return code with missing estimates. Only these aggregates leave a private
* run directory.
args dataset output effects k threads
version 18
clear all
set more off
use "`dataset'", clear
local x
forvalues j = 1/`k' {
    local x `x' x`j'
}
local absorb g1
forvalues j = 2/`effects' {
    local absorb `absorb' g`j'
}
tempname results
postfile `results' double tolerance str32 term double b se N iterations rc using "`output'", replace
foreach tolerance in 1e-14 1e-12 1e-10 1e-9 1e-8 1e-7 1e-6 {
    capture noisily fereg y `x', absorb(`absorb') tolerance(`tolerance') threads(`threads')
    local rc = _rc
    foreach v of local x {
        if `rc' {
            post `results' (`tolerance') ("`v'") (.) (.) (.) (.) (`rc')
        }
        else {
            post `results' (`tolerance') ("`v'") (_b[`v']) (_se[`v']) (e(N)) (e(ic)) (0)
        }
    }
}
postclose `results'
display "ACCURACY_TOLERANCE_COMPLETE"
