version 18.0
clear all
set more off
set seed 314159
set obs 16512
generate long row = _n
generate byte g = mod(_n,17)
generate byte h = mod(floor((_n-1)/17),13)
generate byte cluster = mod(floor((_n-1)/221),19)
generate double w = 1 + mod(_n,7)
generate double y = g/10 + h/20 + rnormal()
forvalues j=1/16 {
    generate double x`j' = rnormal()
    quietly replace y = y + x`j'/`j'
}

// Explicit weighted dummies independently exercise the newly admitted
// coefficient route and each weight kind's distinct covariance scaling.
foreach case in iid_aw iid_fw robust_aw robust_fw robust_pw cluster_aw cluster_fw cluster_pw {
    local weight aw
    if strpos("`case'","_fw") local weight fw
    if strpos("`case'","_pw") local weight pw
    local options
    if strpos("`case'","robust_")==1 local options vce(robust)
    if strpos("`case'","cluster_")==1 local options vce(cluster cluster)
    quietly regress y x1-x16 i.g i.h [`weight'=w], `options'
    matrix oracle_b=e(b)
    matrix oracle_v=e(V)
    scalar oracle_N=e(N)
    scalar oracle_rss=e(rss)
    scalar oracle_df=e(df_r)
    predict double oracle_residual, residuals
    foreach t in 1 4 16 {
        quietly fereg y x1-x16 [`weight'=w], absorb(g h) `options' threads(`t') residuals(native_residual)
        assert row==_n
        assert e(sample)==1
        assert e(N)==oracle_N
        assert e(df_r)==oracle_df
        assert abs(e(rss)-oracle_rss)<1e-8*oracle_rss
        assert abs(native_residual-oracle_residual)<1e-7
        matrix route=e(fereg_solver)
        assert route[1,15]==8
        assert route[1,16]<=.01
        forvalues j=1/16 {
            assert abs(e(b)[1,`j']-oracle_b[1,`j'])<1e-8
            forvalues k=1/16 {
                assert abs(e(V)[`j',`k']-oracle_v[`j',`k'])<1e-9
            }
        }
        drop native_residual
    }
    drop oracle_residual
}

// Literal expansion checks frequency effective N, DoF and HC1 multiplicity.
quietly fereg y x1-x16 [fw=w], absorb(g h) vce(robust) threads(4)
matrix frequency_b=e(b)
matrix frequency_v=e(V)
scalar frequency_N=e(N)
scalar frequency_df=e(df_r)
preserve
expand w
quietly regress y x1-x16 i.g i.h, vce(robust)
assert e(N)==frequency_N
assert e(df_r)==frequency_df
forvalues j=1/16 {
    assert abs(e(b)[1,`j']-frequency_b[1,`j'])<1e-8
    forvalues k=1/16 {
        assert abs(e(V)[`j',`k']-frequency_v[`j',`k'])<1e-9
    }
}
restore

// Common AW/PW rescaling may not change inference or reference fit scales.
foreach weight in aw pw {
    quietly fereg y x1-x16 [`weight'=w], absorb(g h) vce(robust) threads(4)
    matrix scale_b=e(b)
    matrix scale_v=e(V)
    scalar scale_rss=e(rss)
    quietly replace w=w*1024
    quietly fereg y x1-x16 [`weight'=w], absorb(g h) vce(robust) threads(4)
    assert mreldif(e(b),scale_b)<1e-10
    assert mreldif(e(V),scale_v)<1e-10
    assert abs(e(rss)-scale_rss)<1e-10*scale_rss
    quietly replace w=w/1024
}
// Rank fallbacks retain the ordered finite basis and raw FE reference scale.
generate double x17=2*x1
generate double x18=g/8+h/16
quietly regress y x1-x16 i.g i.h [aw=w], vce(robust)
matrix rank_b=e(b)
matrix rank_v=e(V)
scalar rank_df=e(df_r)
foreach t in 1 16 {
    quietly fereg y x1-x18 [aw=w], absorb(g h) vce(robust) threads(`t')
    assert e(rank)==16
    assert e(df_r)==rank_df
    assert e(b)[1,17]==0
    assert e(b)[1,18]==0
    assert e(fereg_status)[18,2]==3
    assert e(fereg_solver)[1,15]==8
    forvalues j=1/16 {
        assert abs(e(b)[1,`j']-rank_b[1,`j'])<1e-8
        forvalues k=1/16 {
            assert abs(e(V)[`j',`k']-rank_v[`j',`k'])<1e-9
        }
    }
}
display as result "END_FIXTURE|ols_weighted_schur|rc=0"
