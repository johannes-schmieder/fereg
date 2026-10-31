*! fereg developer-only OLS manifest adapter 0.5.0 24aug2026
program define _fereg_ols_hidden_numeric_v1, eclass sortpreserve
    version 15.0
    _fereg_ols `0'
    ereturn local cmd "_fereg_ols_hidden_numeric_v1"
    ereturn local cmdline `"_fereg_ols_hidden_numeric_v1 `0'"'
    ereturn local fereg_gate "developer_hidden_numeric_v1"
end
