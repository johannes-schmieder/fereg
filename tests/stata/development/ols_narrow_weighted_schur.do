version 18.0
clear all
set more off
set seed 271828
set obs 16512
generate long row = _n
generate byte g = mod(_n,17)
generate byte h = mod(floor((_n-1)/17),13)
generate byte cluster = mod(floor((_n-1)/221),19)
generate double w = 1 + mod(_n,7)
generate double y = g/10 + h/20 + rnormal()
forvalues j=1/13 {
    generate double x`j' = rnormal()
    quietly replace y = y + x`j'/`j'
}

// Explicit weighted dummies check the newly admitted widths independently of
// the FE solver, including each weight kind's distinct covariance correction.
foreach k in 1 3 6 13 {
    foreach case in iid_aw iid_fw robust_aw robust_fw robust_pw cluster_aw cluster_fw cluster_pw {
        local weight aw
        if strpos("`case'","_fw") local weight fw
        if strpos("`case'","_pw") local weight pw
        local options
        if strpos("`case'","robust_")==1 local options vce(robust)
        if strpos("`case'","cluster_")==1 local options vce(cluster cluster)
        quietly regress y x1-x`k' i.g i.h [`weight'=w], `options'
        matrix oracle_b=e(b)
        matrix oracle_v=e(V)
        scalar oracle_N=e(N)
        scalar oracle_rss=e(rss)
        scalar oracle_df=e(df_r)
        predict double oracle_residual, residuals
        quietly fereg y x1-x`k' [`weight'=w], absorb(left=g right=h) ///
            `options' threads(16) residuals(native_residual)
        assert row==_n
        assert e(sample)==1
        assert e(N)==oracle_N
        assert e(df_r)==oracle_df
        assert abs(e(rss)-oracle_rss)<1e-8*oracle_rss
        assert abs(native_residual-oracle_residual)<1e-7
        assert e(fereg_solver)[1,15]==8
        assert e(fereg_solver)[1,16]<=.01
        forvalues j=1/`k' {
            assert abs(e(b)[1,`j']-oracle_b[1,`j'])<1e-8
            forvalues l=1/`k' {
                assert abs(e(V)[`j',`l']-oracle_v[`j',`l'])<1e-9
            }
        }
        quietly predict double native_xb, xb
        assert abs(y-native_residual-native_xb-left-right)<1e-7
        drop oracle_residual native_residual native_xb left right
    }
}

// Frequency HC1 is checked against actual replicated rows, not aweight meat.
quietly fereg y x1-x3 [fw=w], absorb(g h) vce(robust) threads(16)
matrix frequency_b=e(b)
matrix frequency_v=e(V)
scalar frequency_N=e(N)
scalar frequency_df=e(df_r)
preserve
expand w
quietly regress y x1-x3 i.g i.h, vce(robust)
assert e(N)==frequency_N
assert e(df_r)==frequency_df
forvalues j=1/3 {
    assert abs(e(b)[1,`j']-frequency_b[1,`j'])<1e-8
    forvalues k=1/3 {
        assert abs(e(V)[`j',`k']-frequency_v[`j',`k'])<1e-9
    }
}
restore
foreach weight in aw pw {
    quietly fereg y x1-x3 [`weight'=w], absorb(g h) vce(robust) threads(16)
    matrix scale_b=e(b)
    matrix scale_v=e(V)
    scalar scale_rss=e(rss)
    quietly replace w=w*1024
    quietly fereg y x1-x3 [`weight'=w], absorb(g h) vce(robust) threads(16)
    assert mreldif(e(b),scale_b)<1e-10
    assert mreldif(e(V),scale_v)<1e-10
    assert abs(e(rss)-scale_rss)<1e-10*scale_rss
    quietly replace w=w/1024
}
display as result "END_FIXTURE|ols_narrow_weighted_schur|rc=0"
