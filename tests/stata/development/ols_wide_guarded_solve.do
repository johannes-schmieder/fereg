version 18
clear all
set more off
set seed 20260909
set obs 1400
generate long g = mod(_n, 10)
generate double weight = 1 + mod(_n, 3)
forvalues j = 1/65 {
    generate double x`j' = rnormal()
}
generate double y = 2 + .2*x1 - .1*x2 + g/10 + rnormal()

* An independent explicit-dummy fit checks wide weighted inference and outputs.
quietly regress y x1-x65 i.g [aw=weight]
matrix reference_b = e(b)
matrix reference_v = e(V)
predict double reference_fitted, xb
predict double reference_residual, residuals
quietly fereg y x1-x65 [aw=weight], absorb(saved=g) residuals(resid) memory(fast) threads(7)
assert e(N) == 1400
assert e(rank) == 65
assert e(df_r) == 1325
forvalues j = 1/65 {
    assert abs(e(b)[1,`j'] - reference_b[1,`j']) <= 1e-10
    forvalues k = 1/65 {
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
assert abs(d-saved) <= 1e-10
quietly summarize saved [aw=weight], meanonly
assert abs(r(mean)) <= 1e-10
matrix fast_b = e(b)
matrix fast_v = e(V)
quietly fereg y x1-x65 [aw=weight], absorb(low_saved=g) residuals(low_resid) memory(low) threads(1)
assert mreldif(e(b),fast_b) <= 1e-10
assert mreldif(e(V),fast_v) <= 1e-10
assert abs(low_saved-saved) <= 1e-9
assert abs(low_resid-resid) <= 1e-9

* The shared prepared boundary also serves the strict PPML checkpoint.
generate double count_y = 1 + rpoisson(exp(.2*x1-.1*x2+g/20))
quietly poisson count_y x1-x65 i.g
matrix poisson_b = e(b)
predict double poisson_mu, n
quietly feppml count_y x1-x65, absorb(g) solver(map) separation(none) memory(fast) threads(7) mu(fast_mu)
assert e(converged) == 1
forvalues j = 1/65 {
    assert abs(e(b)[1,`j'] - poisson_b[1,`j']) <= 1e-8
}
assert abs(fast_mu-poisson_mu) <= 1e-8
matrix ppml_b = e(b)
matrix ppml_v = e(V)
quietly feppml count_y x1-x65, absorb(g) solver(map) separation(none) memory(low) threads(1) mu(low_mu)
assert e(converged) == 1
assert mreldif(e(b),ppml_b) <= 1e-8
assert mreldif(e(V),ppml_v) <= 1e-8
assert abs(fast_mu-low_mu) <= 1e-8
display as result "END_FIXTURE|ols_wide_guarded_solve|rc=0"
