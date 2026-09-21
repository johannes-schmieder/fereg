version 19.0
clear all
set more off
set varabbrev off
set linesize 255
args source refs data threads output
local processors = min(`threads', c(processors_lic), c(processors_mach))
set processors `processors'
sysdir set PLUS `"`refs'"'
sysdir set PERSONAL `"`output'"'
capture adopath - SITE
capture adopath - "."
adopath ++ `"`refs'"'
adopath ++ `"`source'/stata"'
adopath ++ `"`source'/dist"'
quietly use `"`data'"', clear
assert _N == 3159720
assert y > 0 & y < .
timer clear 91
timer on 91
capture noisily feppml y x1 x2 x3 x4, absorb(g1 g2) threads(`threads') vce(robust)
local rc = _rc
timer off 91
quietly timer list 91
local seconds = r(t91)
tempname meta values
file open `meta' using `"`output'/metadata.tsv"', write text replace
file write `meta' "rc" _tab "`rc'" _n "seconds" _tab %24.17g (`seconds') _n
file write `meta' "requested_threads" _tab "`threads'" _n
file write `meta' "stata_processors" _tab "`processors'" _n
if !`rc' {
    local scalars : e(scalars)
    foreach scalar_name of local scalars {
        file write `meta' "`scalar_name'" _tab %24.17g (e(`scalar_name')) _n
    }
    assert e(converged) == 1
    assert e(N) == _N
    assert e(sample) == 1
    matrix __vb_b = e(b)
    matrix __vb_V = e(V)
    file open `values' using `"`output'/coefficients.csv"', write text replace
    file write `values' "name,value,se" _n
    forvalues j = 1/4 {
        file write `values' "x`j'," %24.17g (__vb_b[1,`j']) "," %24.17g (sqrt(__vb_V[`j',`j'])) _n
    }
    file close `values'
    file open `values' using `"`output'/covariance.csv"', write text replace
    file write `values' "row,column,value" _n
    forvalues j = 1/4 {
        forvalues h = 1/4 {
            file write `values' "x`j',x`h'," %24.17g (__vb_V[`j',`h']) _n
        }
    }
    file close `values'
    ereturn list
    matrix list e(b), format(%24.17g)
    matrix list e(V), format(%24.17g)
}
file close `meta'
display as result "VENETO_PPML_FIX_COMPLETE threads=`threads' rc=`rc'"
exit `rc'
