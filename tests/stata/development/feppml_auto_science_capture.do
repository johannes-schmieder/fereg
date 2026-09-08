version 18.0
args dataset output controls effects threads tolerance
use `"`dataset'"', clear
local xlist
if `controls' > 0 {
    forvalues j = 1/`controls' {
        local xlist `xlist' x`j'
    }
}
local glist
forvalues j = 1/`effects' {
    local glist `glist' g`j'
}
generate long audit_row = _n
quietly feppml y `xlist', absorb(`glist') vce(robust) separation(none) ///
    solver(auto) threads(`threads') tolerance(`tolerance') mu(audit_mu)
generate byte audit_sample = e(sample)
assert audit_sample == 1
assert y > 0 & y < . & audit_mu > 0 & audit_mu < .
assert `"`e(feppml_solver_requested)'"' == "auto"
assert e(feppml_solver_fallback_count) == 0
file open audit using `"`output'.tsv"', write text
file write audit "scalar" _tab "outer_tolerance" _tab %24.17g (`tolerance') _n
foreach name in N ll deviance rank df_a df_resid converged feppml_threads feppml_solver_fallback_count {
    file write audit "scalar" _tab "`name'" _tab %24.17g (e(`name')) _n
}
foreach name in b V feppml_certificate feppml_solver {
    matrix audit_matrix = e(`name')
    local columns : colfullnames audit_matrix
    file write audit "columns" _tab "`name'" _tab "`columns'" _n
    forvalues i = 1/`=rowsof(audit_matrix)' {
        forvalues j = 1/`=colsof(audit_matrix)' {
            file write audit "matrix" _tab "`name'" _tab "`i'" _tab "`j'" ///
                _tab %24.17g (audit_matrix[`i',`j']) _n
        }
    }
}
file close audit
format y `xlist' audit_mu %24.17g
export delimited audit_row audit_sample y `xlist' `glist' audit_mu ///
    using `"`output'.csv"', datafmt
display as result "END_FIXTURE|feppml_auto_science_capture|rc=0"
