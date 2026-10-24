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
generate double fractional = .125+mod(_n,17)/11
generate double frequency = 1+mod(_n,7)
generate double shuffle = runiform()
sort shuffle
generate long row = _n
generate double y = g/100+h/20+rnormal()
forvalues j=1/6 {
    generate double x`j' = rnormal()
    quietly replace y = y+x`j'/`j'
}

// Repeated pairs and 3,300 left levels make both existing scratch banks large
// enough for weighted edge caching. areg absorbs only g and uses explicit h
// dummies, providing an independent one-FE projection and finite regression.
foreach k in 3 6 {
    foreach case in iid_aw iid_fw robust_aw robust_fw robust_pw cluster_aw cluster_fw cluster_pw {
        local weight aw
        local w fractional
        if strpos("`case'","_fw") {
            local weight fw
            local w frequency
        }
        if strpos("`case'","_pw") local weight pw
        local options
        if strpos("`case'","robust_")==1 local options vce(robust)
        if strpos("`case'","cluster_")==1 local options vce(cluster cl)
        quietly areg y x1-x`k' i.h [`weight'=`w'], absorb(g) `options'
        matrix oracle_b=e(b)
        matrix oracle_v=e(V)
        scalar oracle_N=e(N)
        scalar oracle_rss=e(rss)
        scalar oracle_df=e(df_r)
        predict double oracle_residual, residuals
        quietly fereg y x1-x`k' [`weight'=`w'], absorb(left=g right=h) ///
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
display as result "END_FIXTURE|ols_weighted_edge_cache|rc=0"
