version 18.0
clear all
set more off

* Distinct-employer degrees 1 through 7, with repeated weighted spells.
set obs 180
generate long worker = _n
generate byte degree = 1 + mod(worker - 1, 7)
expand 2 * degree
bysort worker: generate byte period = _n
generate int firm = 1 + mod(worker * 13 + floor((period - 1) / 2) * 7, 23)
generate double weight = 1 + mod(_n, 5) / 3
generate double x1 = sin(_n * .31)
generate double x2 = cos(_n * .17)
generate double y = .7*x1 - .2*x2 + sin(worker*.11) + cos(firm*.3) + ///
    (mod(_n*37, 101) - 50)*.001

* Independent dummy regression checks inference and the recovered projection.
quietly regress y x1 x2 i.worker i.firm [aw=weight]
matrix reference_b = e(b)
matrix reference_v = e(V)
scalar reference_df_r = e(df_r)
predict double reference_residual, residuals
foreach mode in fast low {
    foreach degree in 4 5 6 {
        quietly fereg y x1 x2 [aw=weight], absorb(sw=worker sf=firm) ///
            solver(cmgschur) cmgdegree(`degree') memory(`mode') poolsize(1) ///
            residuals(resid) threads(2) tolerance(1e-10)
        assert e(fereg_solver)[1,1] == 8
        assert e(fereg_solver)[1,2] == 8
        assert e(fereg_cmg_degree) == `degree'
        assert e(df_r) == reference_df_r
        forvalues j = 1/2 {
            assert abs(e(b)[1,`j'] - reference_b[1,`j']) <= 1e-10
            forvalues k = 1/2 {
                assert abs(e(V)[`j',`k'] - reference_v[`j',`k']) <= 1e-10
            }
        }
        predict double xb, xb
        predict double d, d
        assert abs(resid-reference_residual) <= 1e-9
        assert abs(y-xb-d-resid) <= 1e-10
        assert abs(d-sw-sf) <= 1e-10
        drop sw sf resid xb d
    }
}

quietly fereg y x1 x2 [aw=weight], absorb(worker firm) ///
    solver(cmgschur) threads(1) tolerance(1e-10)
assert e(fereg_cmg_degree) == 5
assert e(fereg_solver)[1,2] == 8
matrix saved_b = e(b)
scalar saved_n = e(N)

* Every unsupported option/topology leaves prior results and outputs untouched.
foreach cutoff in 3 7 {
    capture noisily fereg y x1, absorb(sw=worker sf=firm) ///
        solver(cmgschur) cmgdegree(`cutoff') residuals(rejected)
    assert _rc == 198
    assert mreldif(e(b), saved_b) == 0
    assert e(N) == saved_n
}
foreach solver in auto cmg {
    capture noisily fereg y x1, absorb(sw=worker sf=firm) ///
        solver(`solver') cmgdegree(5) residuals(rejected)
    assert _rc == 198
    assert mreldif(e(b), saved_b) == 0
}
foreach effects in "worker" "worker firm period" "worker##c.x2 firm" {
    capture noisily fereg y x1, absorb(`effects') ///
        solver(cmgschur) residuals(rejected)
    assert _rc == 498
    assert mreldif(e(b), saved_b) == 0
}
capture noisily feppml y x1, absorb(worker firm) solver(cmgschur)
assert _rc == 198
assert mreldif(e(b), saved_b) == 0
capture noisily feppml y x1, absorb(worker firm) cmgdegree(5)
assert _rc == 198
assert mreldif(e(b), saved_b) == 0
foreach output in sw sf rejected {
    capture confirm variable `output'
    assert _rc == 111
}

display as result "END_FIXTURE|ols_cmgschur|rc=0"
