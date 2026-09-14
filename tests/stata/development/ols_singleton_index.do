version 18.0
clear all
set more off
set seed 271828
set obs 1200
generate long row = _n
generate long g = mod(_n, 7)
generate long h = mod(floor((_n-1)/7), 5)
// A 200-row path, separate from a stable crossed panel.
replace g = 10 + floor((_n-1001)/2) if _n>1000
replace h = 10 + floor((_n-1000)/2) if _n>1000
generate double x = rnormal()
generate double z = rnormal()
generate double y = 2*x - z + g/20 + h/30 + rnormal()
generate double w = 1 + mod(_n,3)
replace w = 1 if _n>1000
replace w = 2 in 1200
generate long cl = mod(_n,23)
replace cl = . in 20
replace cl = . in 1090
replace x = . in 30
replace x = . in 1110

foreach weight in none aw fw pw {
    local wt
    if "`weight'"!="none" local wt [`weight'=w]
    foreach covariance in robust cluster {
        local vce vce(robust)
        if "`covariance'"=="cluster" local vce vce(cluster cl)
        generate byte expected = row!=1120 & !missing(y,x,z,g,h)
        if "`covariance'"=="cluster" replace expected = 0 if missing(cl)
        generate byte initial = expected
        generate double mass = 1
        if "`weight'"=="fw" replace mass = w
        // Independent alternating-count oracle. fereg receives the original
        // rows; this expected mask is used only by the oracle regression.
        local changed 1
        while `changed' {
            local changed 0
            foreach fe in g h {
                egen double count = total(cond(expected,mass,0)), by(`fe')
                quietly count if expected & count==1
                local changed = `changed' + r(N)
                replace expected = 0 if count==1
                drop count
            }
        }
        sort row
        quietly count if initial & !expected
        assert r(N)>100
        quietly regress y x z i.g i.h if expected `wt', `vce'
        matrix oracle_b=e(b)
        matrix oracle_v=e(V)
        scalar oracle_N=e(N)
        scalar oracle_rss=e(rss)
        scalar oracle_df=e(df_r)
        predict double oracle_residual if expected, residuals
        foreach mode in fast low {
            quietly fereg y x z if row!=1120 `wt', absorb(g h) `vce' ///
                memory(`mode') threads(4) residuals(native_residual)
            assert e(sample)==expected
            assert row==_n
            assert e(N)==oracle_N
            assert e(df_r)==oracle_df
            assert abs(e(rss)-oracle_rss)<1e-8*oracle_rss
            assert abs(native_residual-oracle_residual)<1e-7 if expected
            assert missing(native_residual) if !expected
            forvalues j=1/2 {
                assert abs(e(b)[1,`j']-oracle_b[1,`j'])<1e-8
                forvalues k=1/2 {
                    assert abs(e(V)[`j',`k']-oracle_v[`j',`k'])<1e-9
                }
            }
            drop native_residual
        }
        drop expected initial mass oracle_residual
    }
}
display as result "END_FIXTURE|ols_singleton_index|rc=0"
