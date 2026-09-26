version 18.0
clear all
set more off
set seed 271829
set obs 16512
generate long row = _n
generate byte g = mod(_n,17)
generate byte h = mod(floor((_n-1)/17),13)
generate byte j = mod(floor((_n-1)/221),7)
generate double x = rnormal()
generate double z = rnormal()
generate double y = 2*x-z+g/10+h/20+j/30+rnormal()
generate double w = 1+mod(_n,3)
generate byte cl = mod(_n,23)

// Explicit dummies independently verify transferred two/three-partition rows,
// reordered sources, unchanged one-FE behavior, and composite/weighted fallbacks.
foreach case in two three reverse one composite weighted {
    local absorb g h
    local dummy i.g i.h
    local weight
    if "`case'"=="three" {
        local absorb g h j
        local dummy i.g i.h i.j
    }
    if "`case'"=="reverse" {
        local absorb h g
        local dummy i.h i.g
    }
    if "`case'"=="one" {
        local absorb g
        local dummy i.g
    }
    if "`case'"=="composite" {
        local absorb g#h j
        local dummy i.g#i.h i.j
    }
    if "`case'"=="weighted" local weight [fw=w]
    quietly regress y x z `dummy' `weight', vce(cluster cl)
    matrix oracle_b=e(b)
    matrix oracle_v=e(V)
    scalar oracle_N=e(N)
    scalar oracle_df=e(df_r)
    scalar oracle_rss=e(rss)
    predict double oracle_residual, residuals
    foreach mode in fast low {
        quietly fereg y x z `weight', absorb(`absorb') vce(cluster cl) ///
            memory(`mode') threads(4) residuals(native_residual)
        assert e(sample)==1
        assert row==_n
        assert e(N)==oracle_N
        assert e(df_r)==oracle_df
        assert abs(e(rss)-oracle_rss)<1e-8*oracle_rss
        assert abs(native_residual-oracle_residual)<1e-7
        forvalues a=1/2 {
            assert abs(e(b)[1,`a']-oracle_b[1,`a'])<1e-8
            forvalues b=1/2 {
                assert abs(e(V)[`a',`b']-oracle_v[`a',`b'])<1e-9
            }
        }
        drop native_residual
    }
    drop oracle_residual
}

// A singleton on either partition invalidates the proposed fixed point. The
// removed extreme levels must disappear before final rank and inference.
replace g=100 in 1
replace h=100 in 2
quietly regress y x z i.g i.h if row>2, vce(robust)
matrix oracle_b=e(b)
matrix oracle_v=e(V)
scalar oracle_df=e(df_r)
predict double oracle_residual if row>2, residuals
foreach mode in fast low {
    quietly fereg y x z, absorb(g h) vce(robust) memory(`mode') ///
        threads(1) residuals(native_residual)
    assert e(sample)==(row>2)
    assert e(df_r)==oracle_df
    assert missing(native_residual) if row<=2
    assert abs(native_residual-oracle_residual)<1e-7 if row>2
    forvalues a=1/2 {
        assert abs(e(b)[1,`a']-oracle_b[1,`a'])<1e-8
        forvalues b=1/2 {
            assert abs(e(V)[`a',`b']-oracle_v[`a',`b'])<1e-9
        }
    }
    drop native_residual
}
display as result "END_FIXTURE|ols_multi_topology_transfer|rc=0"
