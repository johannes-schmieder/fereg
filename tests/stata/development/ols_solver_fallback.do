version 18
clear all
set more off

* Certified fallbacks after explicit requests (SPEC §1): a valid request its
* route cannot serve continues on the next certified route, reports the
* request against what ran with the reason, and matches the automatic fit;
* an exhausted budget still fails and keeps the previous results.
set obs 3200
generate long worker = floor((_n-1)/32)
generate long firm = mod(worker + mod(floor((_n-1)/4),8),10)
generate long g3 = mod(worker,7)
generate long g4 = mod(worker,5)
generate long g5 = floor(worker/3)
generate long g6 = mod(worker,11)
generate double t = 1 + mod(_n,7)/3
generate double x = 2*mod(_n-1,2)-1 + .3*sin(worker*.173) + .1*cos(firm*.113)
generate double noise = .01*(2*(mod(_n-1,4)>=2)-1)
generate double y = .7*x + sin(worker*.137) + cos(firm*.291) + noise

* Unsupported for the design: the elimination outside two to five effects,
* CMG with absorbed slopes, CMG-Schur outside two intercept effects.
foreach case in 1 2 3 {
    if `case' == 1 {
        local absorb "worker firm g3 g4 g5 g6"
        local request "cg, reduce(eliminate) precondition(jacobi)"
    }
    if `case' == 2 {
        local absorb "worker firm##c.t"
        local request "cg, precondition(cmg)"
    }
    if `case' == 3 {
        local absorb "worker firm g3"
        local request "cg, reduce(eliminate) precondition(cmg)"
    }
    foreach memory in fast low {
        quietly fereg y x, absorb(`absorb') memory(`memory') poolsize(1) ///
            tolerance(1e-10) residuals(auto_resid)
        scalar auto_b = _b[x]
        fereg y x, absorb(`absorb') solver(`request') memory(`memory') ///
            poolsize(1) tolerance(1e-10) residuals(resid)
        assert reldif(_b[x], auto_b) <= 1e-8
        assert abs(resid - auto_resid) <= 1e-7
        assert `"`e(fe_requested)'"' == `"`request'"'
        assert e(fe_fallback) == 1 & `"`e(fe_fallback_reason)'"' == "unsupported"
        assert `"`e(fe_route)'"' != "" & `"`e(fe_route)'"' != `"`request'"'
        assert `"`e(fe_preconditioner)'"' != "cmg"
        display "FALLBACK|case=`case'|memory=`memory'|requested=`e(fe_requested)'|route=`e(fe_route)'|reason=`e(fe_fallback_reason)'"
        drop resid auto_resid
    }
}

* An explicit request that exhausts iterate() leaves no budget for a
* fallback: it fails with r(430) and keeps the previous results.
quietly fereg y x, absorb(worker firm)
tempname prior_b
matrix `prior_b' = e(b)
capture noisily fereg y x, absorb(worker firm g3) ///
    solver(lsmr, precondition(none)) tolerance(1e-12) iterate(1) residuals(failed)
assert _rc == 430
assert mreldif(e(b), `prior_b') == 0
confirm new variable failed
* Syntax errors are never replaced.
capture noisily fereg y x, absorb(worker firm) solver(cg, reduce(full) precondition(block))
assert _rc == 198
assert mreldif(e(b), `prior_b') == 0

display as result "END_FIXTURE|ols_solver_fallback|rc=0"
