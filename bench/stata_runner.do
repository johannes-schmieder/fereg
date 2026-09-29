version 19.0
clear all
set more off
set varabbrev off
set linesize 255
args config
include `"`config'"'
local processors = min(`threads', `processor_cap', c(processors_lic), c(processors_mach))
set processors `processors'
sysdir set PLUS `"`refs'"'
sysdir set PERSONAL `"`output'"'
capture adopath - SITE
capture adopath - "."
adopath ++ `"`refs'"'
adopath ++ `"`source'/stata"'
adopath ++ `"`source'/dist"'
quietly use `"`dataset'"', clear
local xvars
forvalues j = 1/`k' {
    local xvars `xvars' x`j'
}
local absorb
forvalues j = 1/`effects' {
    local effect g`j'
    if `slopes' & `j' == 1 local effect g1##c.slope
    if `outputs' local effect saved`j'=`effect'
    local absorb `absorb' `effect'
}
local weight
if "`weights'" == "analytic" local weight [aw=weight]
local inference
if "`vce'" == "robust" local inference vce(robust)
if "`vce'" == "cluster" local inference vce(cluster cluster)
local command `arm'
if "`arm'" == "fereg" & "`model'" == "ppml" local command feppml
local controls
if "`arm'" == "fereg" {
    local controls threads(`threads')
    if `engine_option' local controls `controls' engine(`engine')
    if "`tolerance'" != "" local controls `controls' tolerance(`tolerance')
    if "`solver'" != "auto" local controls `controls' solver(`solver')
}
local post
if `outputs' local post residuals(bench_residual)
local command `"`command' y `xvars' `weight', absorb(`absorb') `inference' `controls' `post'"'
timer clear 91
timer on 91
capture noisily quietly `command'
local rc = _rc
timer off 91
quietly timer list 91
local seconds = r(t91)
tempname metadata
file open `metadata' using `"`output'/metadata.tsv"', write text replace
file write `metadata' "rc" _tab "`rc'" _n "seconds" _tab %24.17g (`seconds') _n
file write `metadata' "processors" _tab "`processors'" _n
file write `metadata' "engine" _tab "`engine'" _n
if !`rc' {
    file write `metadata' "N" _tab %24.17g (e(N)) _n
    tempname coeff
    file open `coeff' using `"`output'/coefficients.csv"', write text replace
    file write `coeff' "name,value" _n
    forvalues j = 1/`k' {
        file write `coeff' "x`j'," %24.17g (_b[x`j']) _n
    }
    file close `coeff'
    if "`arm'" == "fereg" & "`model'" == "ols" {
        file write `metadata' "input_seconds" _tab %24.17g (el(e(fereg_runtime),19,2)) _n
        file write `metadata' "native_seconds" _tab %24.17g (el(e(fereg_runtime),20,2)) _n
        file write `metadata' "total_seconds" _tab %24.17g (el(e(fereg_runtime),21,2)) _n
    }
    if "`arm'" == "fereg" & "`model'" == "ppml" {
        file write `metadata' "total_seconds" _tab %24.17g (el(e(feppml_runtime),1,1)) _n
    }
    quietly keep if e(sample)
    quietly keep obs_id
    quietly export delimited using `"`output'/sample.csv"', replace
}
file write `metadata' "complete" _tab "1" _n
file close `metadata'
display "FEREG_BENCH_COMPLETE rc=`rc'"
exit `rc', clear
