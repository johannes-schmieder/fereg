* Coefficient accuracy against tolerance(): descriptive and untimed.
* Fits one linear model at several tolerances, the first a 1e-14 reference,
* and posts every coefficient and standard error. Only these aggregates leave
* a private run directory.
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
postfile `results' double tolerance str32 term double b se N iterations using "`output'", replace
foreach tolerance in 1e-14 1e-12 1e-10 1e-9 1e-8 1e-7 1e-6 {
    fereg y `x', absorb(`absorb') tolerance(`tolerance') threads(`threads')
    foreach v of local x {
        post `results' (`tolerance') ("`v'") (_b[`v']) (_se[`v']) (e(N)) (e(ic))
    }
}
postclose `results'
display "ACCURACY_TOLERANCE_COMPLETE"
