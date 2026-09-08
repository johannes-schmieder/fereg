version 18.0
clear all
set more off
local repository : environment FEREG_PPML_SOLVER_REPOSITORY
local dataset : environment FEREG_PPML_SOLVER_DATASET
local output : environment FEREG_PPML_SOLVER_RESULT
local controls : environment FEREG_PPML_SOLVER_K
local effects : environment FEREG_PPML_SOLVER_EFFECTS
local threads : environment FEREG_PPML_SOLVER_THREADS
local tolerance : environment FEREG_PPML_SOLVER_OUTER_TOLERANCE
local capture_driver : environment FEREG_PPML_SOLVER_CAPTURE_DRIVER
local stata_log : environment FEREG_PPML_SOLVER_STATA_LOG
log using `"`stata_log'"', text
adopath ++ `"`repository'/stata"'
adopath ++ `"`repository'/dist"'
findfile feppml.ado
assert `"`r(fn)'"' == `"`repository'/stata/feppml.ado"'
local original_csv : environment FEREG_PPML_SOLVER_ORIGINAL_CSV
if `"`original_csv'"' != "" {
    use `"`dataset'"', clear
    local expected_n : environment FEREG_PPML_SOLVER_N
    assert _N == `expected_n'
    local original_xlist
    if `controls' > 0 {
        forvalues j = 1/`controls' {
            local original_xlist `original_xlist' x`j'
        }
    }
    format y `original_xlist' %24.17g
    export delimited y `original_xlist' g* using `"`original_csv'"', datafmt
}
capture noisily do `"`capture_driver'"' `"`dataset'"' `"`output'"' ///
    `controls' `effects' `threads' `tolerance'
local rc = _rc
display "AUTO_SCIENCE_DRIVER_RC=`rc'"
exit `rc'
