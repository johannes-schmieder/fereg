version 17.0
clear all
set more off
set obs 4096
generate double x=32 + cond(mod(_n-1,4)<2,-1,1)
generate double u=cond(mod(_n,2)==0,.125,-.125)
generate double y=3 + 2*x + u
generate double w=1
program define _direct_marker
    args name
    local trace : environment FEREG_SOLVER_TRACE_FILE
    if `"`trace'"'!="" {
        file open directtrace using `"`trace'"', write append text
        file write directtrace "FEREG_SOLVER_TRACE|phase=no_fe_direct_case|name=`name'" _n
        file close directtrace
    }
end
quietly regress y x
matrix oracle_b=e(b)
matrix oracle_v=e(V)
foreach t in 1 4 16 {
    _direct_marker eligible_`t'
    quietly fereg y x, threads(`t')
    assert e(fereg_solver)[2,4]==5 & e(fereg_solver)[2,3]==0
    assert e(fereg_solver)[1,3]==1 & e(N_hdfe)==0
    assert mreldif(e(b),oracle_b)<1e-12
    assert mreldif(e(V),oracle_v)<1e-12
    assert abs(e(rss)-64)<1e-10
}
generate byte g=mod(_n,7)
generate byte h=mod(floor(_n/7),3)
quietly regress y x i.g##i.h
matrix factor_b=e(b)
matrix factor_v=e(V)
_direct_marker eligible_factors
quietly fereg y x i.g##i.h, threads(4)
assert e(fereg_solver)[2,4]==5
assert mreldif(e(b),factor_b)<1e-10
assert mreldif(e(V),factor_v)<1e-10
_direct_marker robust
quietly fereg y x, vce(robust) threads(4)
assert e(fereg_solver)[2,4]!=5
_direct_marker weighted
quietly fereg y x [aw=w], threads(4)
assert e(fereg_solver)[2,4]!=5
_direct_marker explicit_map
quietly fereg y x, solver(map) threads(4)
assert e(fereg_solver)[2,4]!=5
_direct_marker low
quietly fereg y x, memory(low) threads(4)
assert e(fereg_solver)[2,4]!=5
_direct_marker residual_output
quietly fereg y x, residuals(saved_r) threads(4)
assert e(fereg_solver)[2,4]!=5
assert abs(saved_r-u)<1e-12
_direct_marker noconstant
quietly fereg y x, noconstant threads(4)
assert e(fereg_solver)[2,4]!=5
display "END_FIXTURE|ols_no_fe_direct|rc=0"
