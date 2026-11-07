*! fereg developer-only OLS transaction adapter 0.5.2 07nov2026
program define _fereg_estimate_ols_test, eclass sortpreserve
    version 15.0
    _fereg_estimate_ols `0'
    ereturn local cmd "_fereg_estimate_ols_test"
    ereturn local cmdline `"_fereg_estimate_ols_test `0'"'
end
