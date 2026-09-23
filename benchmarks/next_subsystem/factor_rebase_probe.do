version 18.0
clear all
set more off
set obs 300
generate long id = _n
generate int g = ceil(_n/20)
generate byte f = mod(_n,3)+1
generate double x = sin(_n*.31)+cos(_n*.17)
generate double y = .8*x+.3*(f==2)-.2*(f==3)+.03*g+sin(_n*.73)
generate byte touse = 1

// The candidate wire cannot distinguish a dynamic default from fixed ib1.
fereg_parse x i.f, touse(touse) absorb(g)
matrix components = r(model_components)
matrix terms = r(model_terms)
local names `"`r(coefficient_names)'"'
local operators `"`r(component_raw_operators)'"'
fereg_parse x ib1.f, touse(touse) absorb(g)
assert mreldif(components,r(model_components)) == 0
assert mreldif(terms,r(model_terms)) == 0
assert `"`names'"' == `"`r(coefficient_names)'"'
assert `"`operators'"' == `"`r(component_raw_operators)'"'
display "FACTOR_REBASE|candidate_metadata=identical"

fvexpand x i.f if f!=1
local default_names `"`r(varlist)'"'
assert `"`default_names'"' == "x 2b.f 3.f"
fvexpand x ib1.f if f!=1
local explicit_names `"`r(varlist)'"'
assert `"`explicit_names'"' == "x 1b.f 2.f 3.f"
display "FACTOR_REBASE|default_final=`default_names'|explicit_final=`explicit_names'"

quietly fereg y x i.f, absorb(g) vce(robust)
matrix accepted_b = e(b)
matrix accepted_V = e(V)
generate byte accepted_sample = e(sample)
replace y = . if f==1
foreach rhs in i.f ib1.f ib2.f ib3.f ibn.f {
    capture quietly fereg y x `rhs', absorb(g) vce(robust) residuals(probe_resid)
    local rc = _rc
    if `rc' {
        assert mreldif(e(b),accepted_b) == 0
        assert mreldif(e(V),accepted_V) == 0
        assert e(sample) == accepted_sample
        confirm new variable probe_resid
    }
    else {
        matrix accepted_b = e(b)
        matrix accepted_V = e(V)
        replace accepted_sample = e(sample)
        drop probe_resid
    }
    display "FACTOR_REBASE|rhs=`rhs'|native_rc=`rc'"
    quietly regress y x `rhs' i.g, vce(robust)
    local oracle_names : colfullnames e(b)
    display "FACTOR_REBASE|rhs=`rhs'|dummy_names=`oracle_names'"
    // Restore a successful native result for the next rollback check.
    quietly fereg y x ib1.f, absorb(g) vce(robust)
    matrix accepted_b = e(b)
    matrix accepted_V = e(V)
    replace accepted_sample = e(sample)
}
display "END_FIXTURE|factor_rebase_probe|rc=0"
