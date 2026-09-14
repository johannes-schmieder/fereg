version 18.0
clear all
set more off
set seed 314159
set obs 604
generate long row = _n
generate long g = 1 + mod(_n-1,10)
generate long h = 1 + mod(floor((_n-1)/10),10)
replace g = 1000 + floor((_n-401)/2) in 401/600
replace h = 1000 + floor((_n-400)/2) in 401/600
replace g = 2000 + floor((_n-601)/2) in 601/604
replace h = 2000 + mod(_n-601,2) in 601/604
generate byte f = mod(floor((_n-1)/100),2)
generate double x = rnormal()
generate double y = 1 + mod(_n,7)
replace y = 0 in 601/602
generate double w = 1 + mod(_n,3)
replace w = 1 if row>400
generate long cl = 1 + mod(_n,23)
replace cl = . in 17
// Rows 401:600 form an initial singleton cascade. The final rectangle has
// no initial singleton; its zero-only FE drops leave two further singletons.
generate byte expected = row<=400 & row!=17
foreach weight in none fw pw {
    local wt
    if "`weight'"!="none" local wt [`weight'=w]
    quietly poisson y x i.g i.h if expected `wt', vce(cluster cl) iterate(200) tolerance(1e-10)
    scalar oracle_b = _b[x]
    predict double oracle_mu if expected, n
    foreach mode in fast low {
        quietly feppml y x `wt', absorb(g h) vce(cluster cl) memory(`mode') ///
            threads(4) separation(fe) mu(native_mu) dropreason(native_drop)
        assert e(sample)==expected
        assert e(N_physical)==399
        assert abs(_b[x]-oracle_b)<1e-8
        assert abs(native_mu-oracle_mu)<1e-7 if expected
        assert missing(native_mu) if !expected
        assert native_drop==8 if inrange(row,401,600) | inrange(row,603,604)
        assert native_drop==6 in 17
        assert native_drop==9 in 601/602
        assert row==_n
        matrix native_b=e(b)
        matrix native_v=e(V)
        matrix native_status=e(feppml_status)
        scalar native_rank=e(rank)
        scalar native_df_a=e(df_a)
        scalar native_df_resid=e(df_resid)
        scalar native_sumweights=e(sumweights)
        assert e(num_singletons)==202
        assert e(separated)==2
        // Same estimator on the independently known fixed point verifies
        // covariance, rank/status, and final normalization after both cascades.
        quietly feppml y x if expected `wt', absorb(g h) vce(cluster cl) ///
            memory(`mode') threads(4) separation(none)
        assert mreldif(e(b),native_b)<1e-10
        assert mreldif(e(V),native_v)<1e-10
        assert mreldif(e(feppml_status),native_status)==0
        assert e(rank)==native_rank
        assert e(df_a)==native_df_a
        assert e(df_resid)==native_df_resid
        assert e(sumweights)==native_sumweights
        drop native_mu native_drop
    }
    drop oracle_mu
}
// Interaction partitions must use their own codes and membership, not raw g.
foreach mode in fast low {
    quietly poisson y x i.g#i.f i.h if expected, vce(cluster cl) iterate(200) tolerance(1e-10)
    scalar oracle_b = _b[x]
    quietly feppml y x, absorb(g#f h) vce(cluster cl) memory(`mode') ///
        threads(4) separation(fe) dropreason(native_drop)
    assert e(sample)==expected
    assert abs(_b[x]-oracle_b)<1e-8
    assert row==_n
    drop native_drop
}
display as result "END_FIXTURE|feppml_singleton_index|rc=0"
