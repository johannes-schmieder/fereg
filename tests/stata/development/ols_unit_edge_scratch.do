version 18.0
clear all
set more off
set seed 104729
set obs 16500
generate long g = floor((_n-1)/5)+1
generate byte position = mod(_n-1,5)
generate byte h = mod(g*17,13)
replace h = mod(g*389+position*97,13) if position>=3
generate long cl = mod(_n,101)
generate double shuffle = runiform()
sort shuffle
generate long row = _n
generate double y = g/100+h/20+rnormal()
forvalues j=1/12 {
    generate double x`j' = rnormal()
    quietly replace y = y+x`j'/`j'
}

// K=3/5/6 exercises the idle-left-RHS cache; K=1 rejects it and K=12
// uses the existing two-bank path. areg plus explicit h dummies supplies an
// independent projection, covariance and saved-output reconstruction check.
foreach k in 1 3 5 6 12 {
    foreach case in iid robust cluster {
        local options
        if "`case'"=="robust" local options vce(robust)
        if "`case'"=="cluster" local options vce(cluster cl)
        quietly areg y x1-x`k' i.h, absorb(g) `options'
        matrix oracle_b=e(b)
        matrix oracle_v=e(V)
        scalar oracle_N=e(N)
        scalar oracle_rss=e(rss)
        scalar oracle_df=e(df_r)
        predict double oracle_residual, residuals
        quietly fereg y x1-x`k', absorb(left=g right=h) ///
            `options' threads(16) residuals(native_residual)
        assert row==_n
        assert e(sample)==1
        assert e(N)==oracle_N
        assert e(df_r)==oracle_df
        assert abs(e(rss)-oracle_rss)<1e-8*oracle_rss
        assert abs(native_residual-oracle_residual)<1e-7
        assert e(fereg_solver)[1,1]==1
        assert e(fereg_solver)[1,2]==4
        assert e(fereg_solver)[1,11]==0
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
display as result "END_FIXTURE|ols_unit_edge_scratch|rc=0"
