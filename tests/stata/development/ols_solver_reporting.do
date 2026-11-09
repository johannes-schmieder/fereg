version 18
clear all
set more off

* Four observations per occupied cell: the within-cell part of x and noise
* sum to zero and are orthogonal. Additive worker/firm terms in x are absorbed.
* The independent answer is b=.7, u=noise.
foreach graph in dense ring {
    clear
    if "`graph'" == "dense" {
        set obs 3200
        generate long worker = floor((_n-1)/32)
        generate long firm = mod(worker + mod(floor((_n-1)/4),8),10)
        local preconditioner jacobi
    }
    else {
        * The ring's Jacobi recurrence stalls; the exact elimination escalates
        * to block-Jacobi with a CMG solve of the retained firm block.
        set obs 4096
        generate long worker = floor((_n-1)/8)
        generate long firm = mod(worker + mod(floor((_n-1)/4),2),512)
        local preconditioner block
    }
    generate double x = 2*mod(_n-1,2)-1 + .3*sin(worker*.173) + .1*cos(firm*.113)
    generate double noise = .01*(2*(mod(_n-1,4)>=2)-1)
    generate double y = .7*x + sin(worker*.137) + cos(firm*.291) + noise
    foreach memory in fast low {
        foreach workers in 1 4 {
            fereg y x, absorb(worker firm) memory(`memory') poolsize(1) ///
                threads(`workers') residuals(resid)
            assert abs(_b[x]-.7) <= 1e-10
            assert abs(resid-noise) <= 1e-8
            assert `"`e(fe_route)'"' == ///
                "cg, reduce(eliminate) precondition(`preconditioner')"
            assert "`e(fe_reduction)'" == "eliminate" & "`e(fe_method)'" == "cg"
            assert "`e(fe_preconditioner)'" == "`preconditioner'"
            assert "`e(fe_acceleration)'" == "none"
            assert e(fe_iterations) == e(ic) & e(fe_iterations) > 0
            if "`preconditioner'" == "block" {
                assert `"`e(fe_local_solvers)'"' == "firm=cmg"
                assert e(fe_escalations) >= 1
            }
            else {
                assert `"`e(fe_local_solvers)'"' == ""
                assert e(fe_escalations) == 0
            }
            assert e(fe_fallback) == 0 & e(fe_residual_fallback) == 0
            assert inlist("`e(fe_residual_method)'", "cg", "derived")
            matrix report = e(fe_solve)
            local rows : rownames report
            assert "`rows'" == "absorption residual"
            assert report[1,1] == e(fe_iterations) & report[2,1] == e(fe_residual_iterations)
            assert "`e(fe_solver)'" == "auto"
            drop resid
        }
    }
}
* Six effects (four nested in the worker) are outside the elimination's
* range: Auto runs CG on the full system (Jacobi, escalating to additive
* Schwarz when slow), the same projection as worker-firm, so the answer is
* still b=.7, u=noise.
clear
set obs 3200
generate long worker = floor((_n-1)/32)
generate long firm = mod(worker + mod(floor((_n-1)/4),8),10)
generate long g3 = mod(worker,7)
generate long g4 = mod(worker,5)
generate long g5 = floor(worker/3)
generate long g6 = mod(worker,11)
generate double x = 2*mod(_n-1,2)-1 + .3*sin(worker*.173) + .1*cos(firm*.113)
generate double noise = .01*(2*(mod(_n-1,4)>=2)-1)
generate double y = .7*x + sin(worker*.137) + cos(firm*.291) + noise
foreach memory in fast low {
    foreach workers in 1 4 {
        fereg y x, absorb(worker firm g3 g4 g5 g6) memory(`memory') poolsize(1) ///
            threads(`workers') residuals(resid)
        assert abs(_b[x]-.7) <= 1e-10
        assert abs(resid-noise) <= 1e-8
        assert "`e(fe_reduction)'" == "full" & "`e(fe_method)'" == "cg"
        if "`e(fe_preconditioner)'" == "schwarz" {
            assert `"`e(fe_route)'"' == "cg, reduce(full) precondition(schwarz)"
            assert `"`e(fe_local_solvers)'"' != "" & e(fe_subdomains) >= 5
            assert e(fe_escalations) >= 1
        }
        else {
            assert `"`e(fe_route)'"' == "cg, reduce(full) precondition(jacobi)"
            assert `"`e(fe_local_solvers)'"' == "" & e(fe_escalations) == 0
        }
        assert e(fe_fallback) == 0
        assert "`e(fe_solver)'" == "auto"
        drop resid
    }
}
* Explicit requests run as named and report what ran.
foreach spec in "lsmr" "lsmr, precondition(jacobi)" "lsmr, precondition(none)" ///
    "cg" "cg, reduce(full) precondition(jacobi)" "cg, precondition(schwarz)" {
    fereg y x, absorb(worker firm g3 g4 g5 g6) solver(`spec') residuals(resid)
    assert abs(_b[x]-.7) <= 1e-9
    assert abs(resid-noise) <= 1e-8
    assert `"`e(fe_solver)'"' == `"`spec'"'
    assert "`e(fe_reduction)'" == "full"
    local method = cond(substr(`"`spec'"', 1, 4) == "lsmr", "lsmr", "cg")
    assert "`e(fe_method)'" == "`method'"
    if strpos(`"`spec'"', "jacobi") assert "`e(fe_preconditioner)'" == "jacobi"
    if strpos(`"`spec'"', "none") assert "`e(fe_preconditioner)'" == "none"
    if `"`spec'"' == "cg, precondition(schwarz)" {
        assert "`e(fe_preconditioner)'" == "schwarz"
        assert `"`e(fe_local_solvers)'"' != "" & e(fe_subdomains) >= 5
        assert e(fe_escalations) == 0
    }
    if inlist(`"`spec'"', "lsmr", "cg") {
        assert inlist("`e(fe_preconditioner)'", "jacobi", "schwarz")
        assert ("`e(fe_preconditioner)'" == "schwarz") == (e(fe_escalations) >= 1)
    }
    assert e(fe_fallback) == 0 & e(fe_residual_fallback) == 0
    * Explicit requests keep the certified final projection.
    assert "`e(fe_residual_method)'" == "`method'"
    drop resid
}
* The elimination needs two to five effects; an explicit request outside its
* range fails without substitution.
capture noisily fereg y x, absorb(worker firm g3 g4 g5 g6) ///
    solver(cg, reduce(eliminate))
assert _rc == 498
quietly fereg y x, absorb(worker firm) solver(cg, reduce(eliminate) precondition(block))
assert `"`e(fe_route)'"' == "cg, reduce(eliminate) precondition(block)"
assert `"`e(fe_local_solvers)'"' != "" & e(fe_escalations) == 0
foreach request in "solver(auto, precondition(jacobi))" ///
    "solver(cg, reduce(full) precondition(block))" ///
    "solver(cg, reduce(eliminate) precondition(schwarz))" {
    capture noisily fereg y x, absorb(worker firm) `request'
    assert _rc == 198
}
* A one-effect projection is exact (direct); explicit MAP is reported as MAP.
quietly fereg y x, absorb(worker)
assert "`e(fe_route)'" == "direct" & "`e(fe_reduction)'" == "full"
assert e(fe_fallback) == 0
quietly fereg y x, absorb(worker) solver(map)
assert "`e(fe_route)'" == "map" & "`e(fe_solver)'" == "map"
display as result "END_FIXTURE|ols_solver_reporting|rc=0"
