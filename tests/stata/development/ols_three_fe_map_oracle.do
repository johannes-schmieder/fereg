version 18.0
clear all
set more off
set seed 2026101302
set obs 4000
generate long g1 = mod(_n-1,8)
generate long g2 = mod(floor((_n-1)/8),10)
generate long g3 = mod(floor((_n-1)/80),5)
forvalues j=1/3 {
    generate double x`j' = rnormal()
}
generate double y = .7*x1-.3*x2+.1*x3+g1/8-g2/10+g3/5+rnormal()

* Explicit diagonal preconditioning retains the geometry-led MAP policy.
* Full dummy QR is independent of that FE projection.
quietly regress y x1-x3 i.g1 i.g2 i.g3, vce(robust)
matrix oracle_b = e(b)
matrix oracle_v = e(V)
scalar oracle_df = e(df_r)
predict double oracle_fitted, xb
predict double oracle_residual, residuals
foreach mode in fast low {
    foreach t in 1 4 {
        quietly fereg y x1-x3, absorb(s1=g1 s2=g2 s3=g3) ///
            preconditioner(diagonal) memory(`mode') poolsize(1) threads(`t') ///
            vce(robust) residuals(resid)
        assert e(N) == 4000
        assert e(rank) == 3
        assert e(df_r) == oracle_df
        assert e(fereg_solver)[1,2] == 2
        assert e(fereg_solver)[1,11] == 0
        assert e(fereg_solver)[2,2] == 3
        forvalues j=1/3 {
            assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-9
            forvalues k=1/3 {
                assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
            }
        }
        predict double fitted, xbd
        predict double xb, xb
        predict double d, d
        assert abs(fitted-oracle_fitted) < 1e-9
        assert abs(resid-oracle_residual) < 1e-9
        assert abs(y-xb-d-resid) < 1e-9
        assert abs(d-s1-s2-s3) < 1e-9
        drop s1 s2 s3 resid fitted xb d
    }
}
display as result "END_FIXTURE|ols_three_fe_map_oracle|rc=0"
