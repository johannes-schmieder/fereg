version 18
clear all
set more off
set seed 2026090902
set obs 1400
generate long g1 = mod(_n, 31)
generate long g2 = mod(floor((_n-1)/31), 17)
generate long g3 = mod(_n*7 + floor(_n/13), 11)
generate double weight = 1 + mod(_n, 3)
forvalues j = 1/10 {
    generate double x`j' = rnormal()
}
generate double y = 2 + .2*x1 - .1*x2 + g1/10 - g2/8 + g3/6 + rnormal()

* Explicit dummies independently check weighted inference and saved FE outputs.
quietly regress y x1-x10 i.g1 i.g2 i.g3 [aw=weight]
matrix reference_b = e(b)
matrix reference_v = e(V)
scalar reference_df_r = e(df_r)
predict double reference_fitted, xb
predict double reference_residual, residuals
foreach pool in 1 5 10 {
    quietly fereg y x1-x10 [aw=weight], absorb(s1=g1 s2=g2 s3=g3) residuals(resid) memory(low) poolsize(`pool') solver(lsmr) preconditioner(schwarz) tolerance(1e-10) threads(7)
    assert e(N) == 1400
    assert e(rank) == 10
    assert e(df_r) == reference_df_r
    assert e(fereg_solver)[1,2] == 3
    forvalues j = 1/10 {
        assert abs(e(b)[1,`j'] - reference_b[1,`j']) <= 1e-10
        forvalues k = 1/10 {
            assert abs(e(V)[`j',`k'] - reference_v[`j',`k']) <= 1e-10
        }
    }
    predict double fitted, xbd
    predict double xb, xb
    predict double d, d
    predict double predicted_residual, residuals
    assert abs(fitted-reference_fitted) <= 1e-9
    assert abs(resid-reference_residual) <= 1e-9
    assert abs(predicted_residual-resid) <= 1e-10
    assert abs(y-xb-d-resid) <= 1e-10
    assert abs(d-s1-s2-s3) <= 1e-10
    foreach saved in s1 s2 s3 {
        quietly summarize `saved' [aw=weight], meanonly
        assert abs(r(mean)) <= 1e-10
    }
    if `pool' == 1 {
        matrix low_b = e(b)
        matrix low_v = e(V)
        generate double low_resid = resid
    }
    else {
        assert mreldif(e(b),low_b) <= 1e-10
        assert mreldif(e(V),low_v) <= 1e-10
        assert abs(resid-low_resid) <= 1e-9
    }
    drop s1 s2 s3 resid fitted xb d predicted_residual
}
quietly fereg y x1-x10 [aw=weight], absorb(g1 g2 g3) residuals(fast_resid) memory(fast) solver(lsmr) preconditioner(schwarz) tolerance(1e-10) threads(1)
assert mreldif(e(b),low_b) <= 1e-10
assert mreldif(e(V),low_v) <= 1e-10
assert abs(fast_resid-low_resid) <= 1e-9
display as result "END_FIXTURE|ols_lsmr_reuse|rc=0"
