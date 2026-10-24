version 18
clear all
set more off

* Four observations per occupied cell: x and noise each sum to zero,
* and x is orthogonal to noise. The independent answer is b=.7, u=noise.
foreach graph in dense ring {
    clear
    if "`graph'" == "dense" {
        set obs 128
        generate long worker = floor((_n-1)/16)
        generate long firm = mod(floor((_n-1)/4),4)
        local expected cg
        local code 4
    }
    else {
        set obs 4096
        generate long worker = floor((_n-1)/8)
        generate long firm = mod(worker + mod(floor((_n-1)/4),2),512)
        local expected cmgschur
        local code 8
    }
    generate double x = 2*mod(_n-1,2)-1
    generate double noise = .01*(2*(mod(_n-1,4)>=2)-1)
    generate double y = .7*x + sin(worker*.137) + cos(firm*.291) + noise
    foreach memory in fast low {
        foreach workers in 1 4 {
            fereg y x, absorb(worker firm) memory(`memory') poolsize(1) ///
                threads(`workers') residuals(resid)
            assert abs(_b[x]-.7) <= 1e-10
            assert abs(resid-noise) <= 1e-8
            assert rowsof(e(fereg_solver)) == 2
            assert colsof(e(fereg_solver)) == 16
            assert e(fereg_solver)[1,2] == `code'
            assert "`e(fereg_solver_used)'" == "`expected'"
            local names auto map lsmr cg it squarem cmg cmgschur
            local residual_code = e(fereg_solver)[2,2]
            local residual_name : word `residual_code' of `names'
            assert "`e(fereg_solver_residual)'" == "`residual_name'"
            assert e(fereg_solver_fallback) == ///
                max(e(fereg_solver)[1,11],e(fereg_solver)[2,11])
            assert e(fereg_solver_fallback) == 0
            matrix report = e(fereg_solver)
            local rows : rownames report
            assert "`rows'" == "absorption residual"
            drop resid
        }
    }
}
* A direct one-effect certificate retains MAP and needs no CG recurrence.
quietly fereg y x, absorb(worker)
assert "`e(fereg_solver_used)'" == "map"
assert e(fereg_solver)[1,2] == 2
assert e(fereg_solver_fallback) == 0
display as result "END_FIXTURE|ols_solver_reporting|rc=0"
