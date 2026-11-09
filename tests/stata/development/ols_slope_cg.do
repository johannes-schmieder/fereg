version 18
clear all
set more off

* Explicit solver(cg) on absorbed slopes (fedemean's PCG): every reduction and
* preconditioner of the grammar agrees with a tightly converged automatic
* fit, reports what ran, and a choice that does not serve slopes continues on
* the certified fallback (reason unsupported). Slope t varies within each
* firm; firm 0's slope is constant (a singular level block).
set obs 2400
generate long worker = floor((_n-1)/6)
generate int year = mod(_n-1,6)
generate long firm = mod(worker + floor(year/2), 37)
generate double t = 1 + year + mod(_n*7,5)/4
replace t = 2.5 if firm == 0
generate double x = sin(_n*.31) + .2*t
generate double y = .7*x + sin(worker*.11) + .05*firm*t + .1*year + ///
    (mod(_n*37,101)-50)*.001

foreach absorb in "worker firm##c.t" "worker##c.t firm" "worker firm##c.t year" {
    quietly fereg y x, absorb(`absorb') tolerance(1e-12) residuals(reference)
    scalar reference_b = _b[x]
    scalar reference_se = _se[x]
    local effects : word count `absorb'
    foreach request in "cg" "cg, reduce(full)" "cg, reduce(full) precondition(none)" ///
        "cg, reduce(full) precondition(jacobi)" "cg, precondition(schwarz)" ///
        "cg, reduce(eliminate)" "cg, reduce(eliminate) precondition(jacobi)" ///
        "cg, precondition(block)" {
        foreach workers in 1 4 {
            quietly fereg y x, absorb(`absorb') solver(`request') ///
                tolerance(1e-10) threads(`workers') residuals(actual)
            assert reldif(_b[x], reference_b) <= 1e-8
            assert reldif(_se[x], reference_se) <= 1e-6
            assert abs(actual - reference) <= 1e-7
            assert `"`e(fe_requested)'"' == `"`request'"'
            assert `"`e(fe_requested_method)'"' == "cg"
            assert "`e(fe_method)'" == "cg"
            local unsupported = strpos(`"`request'"', "block") > 0 | ///
                (strpos(`"`request'"', "eliminate") > 0 & `effects' != 2)
            * Unpreconditioned CG may stall on slope channels: it then
            * continues on the automatic full-system CG (nonconvergence).
            local stalled = strpos(`"`request'"', "none") > 0 & e(fe_fallback) == 1
            if `stalled' {
                assert `"`e(fe_fallback_reason)'"' == "nonconvergence"
                assert "`e(fe_preconditioner)'" != "none"
            }
            else assert e(fe_fallback) == `unsupported'
            if `stalled' {
            }
            else if `unsupported' {
                assert `"`e(fe_fallback_reason)'"' == "unsupported"
                assert "`e(fe_reduction)'" == "full"
            }
            else {
                assert `"`e(fe_fallback_reason)'"' == "none"
                if strpos(`"`request'"', "eliminate") assert "`e(fe_reduction)'" == "eliminate"
                if strpos(`"`request'"', "full") assert "`e(fe_reduction)'" == "full"
                if strpos(`"`request'"', "none") assert "`e(fe_preconditioner)'" == "none"
                if strpos(`"`request'"', "jacobi") assert "`e(fe_preconditioner)'" == "jacobi"
                if strpos(`"`request'"', "schwarz") assert "`e(fe_preconditioner)'" == "schwarz"
            }
            display "SLOPE_CG|absorb=`absorb'|request=`request'|workers=`workers'|route=`e(fe_route)'|fallback=`e(fe_fallback_reason)'|it=" e(fe_iterations)
            drop actual
        }
    }
    drop reference
}

* Saved slope effects from an explicit CG fit add up to the absorbed sum.
fereg y x, absorb(fw=worker ff=firm##c.t) solver(cg, reduce(full)) ///
    tolerance(1e-10) residuals(resid)
predict double dsum, d
assert abs(dsum - (fw + ff + ff_Slope1)) <= 1e-8
summarize ff, meanonly
assert abs(r(mean)) <= 1e-8

display as result "END_FIXTURE|ols_slope_cg|rc=0"
