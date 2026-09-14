version 18.0
clear all
set more off
set seed 271828182
set obs 131072
generate long row=_n
generate byte g=mod(_n,8)
generate int cl=mod(_n,97)
generate double x1=cos(_n*.111)+rnormal()/5
generate double x2=x1+.02*rnormal()
generate double y=1.25*x1-.75*x2+.2*g+.01*rnormal()
generate double w=1+mod(_n,5)
replace x1=. in 17
replace cl=. in 29
foreach weight in none aw fw pw {
    local wt
    local vce
    if "`weight'"!="none" {
        local wt [`weight'=w]
        local vce vce(robust)
    }
    if "`weight'"=="pw" local vce vce(cluster cl)
    quietly regress y x1 x2 i.g `wt', `vce'
    matrix oracle_b=e(b)
    matrix oracle_v=e(V)
    scalar oracle_N=e(N)
    scalar oracle_df=e(df_r)
    scalar oracle_rss=e(rss)
    generate byte expected=e(sample)
    predict double oracle_residual if expected, residuals
    foreach mode in fast low {
        quietly fereg y x1 x2 `wt', absorb(g) `vce' memory(`mode') ///
            threads(4) residuals(native_residual)
        assert e(sample)==expected
        assert e(N)==oracle_N
        assert e(df_r)==oracle_df
        assert abs(e(rss)-oracle_rss)<1e-9*oracle_rss
        assert abs(native_residual-oracle_residual)<1e-8 if expected
        assert missing(native_residual) if !expected
        assert row==_n
        forvalues j=1/2 {
            assert abs(e(b)[1,`j']-oracle_b[1,`j'])<1e-8
            forvalues k=1/2 {
                assert abs(e(V)[`j',`k']-oracle_v[`j',`k'])<1e-10
            }
        }
        drop native_residual
    }
    drop expected oracle_residual
}
display as result "END_FIXTURE|ols_qr_reductions|rc=0"
