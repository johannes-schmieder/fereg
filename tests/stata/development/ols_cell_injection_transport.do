version 18.0
clear all
set more off

* Frontend transport and publication coverage. Actual cell construction and
* deferred commitment are checked separately by native diagnostics.
set obs 120
generate long worker = _n
expand 4
bysort worker: generate byte period = _n
generate int firm = 1 + mod(worker + period, 31)
generate double x = mod((_n-1)*37 + 11, 997)/97 - 5
generate double y = .7*x + sin(worker*.13) + cos(firm*.29) + ///
    (mod((_n-1)*53 + 7, 101)-50)*.001
quietly regress y x i.worker i.firm, vce(robust)
tempname reference_b reference_v baseline_b baseline_v
scalar `reference_b' = _b[x]
scalar `reference_v' = _se[x]^2
predict double reference_residual, residuals

foreach workers in 1 4 {
    quietly _fereg_estimate_ols_test y x, absorb(worker firm) ///
        vce(robust) memory(fast) threads(`workers') residual(baseline_residual)
    matrix `baseline_b' = e(b)
    matrix `baseline_v' = e(V)
    foreach mode in eager deferred {
        foreach suffix in none moments scores both {
            local token cells_`mode'
            if "`suffix'" != "none" local token `token'_`suffix'
            quietly _fereg_estimate_ols_test y x, absorb(worker firm) ///
                vce(robust) memory(fast) threads(`workers') ///
                residual(cell_residual) inject(`token')
            assert e(converged) == 1 & e(N) == _N
            assert e(sample) == 1
            assert e(fereg_receipt)[1,16] == 1
            assert abs(_b[x]-`reference_b') <= 1e-10 + 1e-7*abs(`reference_b')
            assert abs(_se[x]^2-`reference_v') <= 1e-10 + 1e-6*`reference_v'
            assert abs(cell_residual-reference_residual) <= ///
                1e-9 + 1e-7*abs(reference_residual)
            assert mreldif(e(b),`baseline_b') <= 1e-7
            assert mreldif(e(V),`baseline_v') <= 1e-6
            drop cell_residual
        }
    }
    drop baseline_residual
}

* Ineligible requests fail before publication and preserve the last fit.
matrix `baseline_b' = e(b)
matrix `baseline_v' = e(V)
foreach request in "solver(lsmr)" "preconditioner(off)" {
    capture noisily _fereg_estimate_ols_test y x, absorb(worker firm) ///
        `request' inject(cells_deferred_both) residual(failed_residual)
    assert _rc == 198
    confirm new variable failed_residual
    assert mreldif(e(b),`baseline_b') == 0
    assert mreldif(e(V),`baseline_v') == 0
    assert e(sample) == 1
}
capture noisily _fereg_estimate_ols_test y x, absorb(worker firm) ///
    inject(cells_deferred_unknown) residual(failed_residual)
assert _rc == 198
confirm new variable failed_residual
assert mreldif(e(b),`baseline_b') == 0
assert mreldif(e(V),`baseline_v') == 0

di as result "END_FIXTURE|ols_cell_injection_transport|rc=0"
