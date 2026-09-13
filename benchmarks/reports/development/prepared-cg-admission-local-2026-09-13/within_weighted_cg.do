version 18.0
clear all
set more off
set seed 20260906
set obs 100000
generate long g1 = mod(_n-1,20)
generate long g2 = mod(floor((_n-1)/20)+7*(_n-1),16)
generate double x1 = rnormal()
generate double x2 = rnormal()
generate double y = 0.7*x1 - 0.3*x2 + g1/30 + g2/40 + rnormal()
generate double w = 0.1*(1+mod(_n,17))
quietly regress y x1 x2 i.g1 i.g2 [aw=w], vce(robust)
matrix oracle_b = e(b)
matrix oracle_v = e(V)
scalar oracle_rss = e(rss)
scalar oracle_df = e(df_r)
predict double oracle_residual, residuals
foreach t in 1 7 {
    fereg y x1 x2 [aw=w], absorb(g1 g2) solver(cg) threads(`t') vce(robust) residuals(native_residual)
    assert e(fereg_certificate)[32,2] == 1
    assert e(df_r) == oracle_df
    forvalues j=1/2 {
        assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-7
        forvalues k=1/2 {
            assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
        }
    }
    assert abs(e(rss)-oracle_rss) <= 1e-7*oracle_rss
    assert abs(native_residual-oracle_residual) <= 1e-7
    drop native_residual
}
display as result "END_FIXTURE|within_weighted_cg|rc=0"
