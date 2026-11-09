version 18.0
clear all
set more off
set seed 1299721
set obs 16416
generate long g1 = mod(_n-1,64)
generate long g2 = mod(17*(_n-1)+floor((_n-1)/64),47)
forvalues j=1/50 {
    generate double x`j' = rnormal()
}
generate double y = 0.7*x1 - 0.3*x2 + g1/30 + g2/40 + rnormal()
generate double w = 0.1*(1+mod(_n,17))
foreach effects in one two {
    local absorb g1
    local dummies i.g1
    if "`effects'" == "two" {
        local absorb g1 g2
        local dummies i.g1 i.g2
    }
    foreach weighted in 0 1 {
        local weight
        if `weighted' local weight [aw=w]
        quietly regress y x1-x50 `dummies' `weight', vce(robust)
        matrix oracle_b = e(b)
        matrix oracle_v = e(V)
        scalar oracle_rss = e(rss)
        scalar oracle_df = e(df_r)
        foreach t in 1 4 16 {
            quietly fereg y x1-x50 `weight', absorb(`absorb') threads(`t') vce(robust)
            assert e(fereg_certificate)[32,2] == 1
            assert e(df_r) == oracle_df
            forvalues j=1/50 {
                assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-8
                forvalues k=1/50 {
                    assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
                }
            }
            assert abs(e(rss)-oracle_rss) <= 1e-9*oracle_rss
            if "`effects'" == "two" & `weighted' == 0 {
                * Auto serves this design by the exact elimination (code 9).
                assert "`e(fe_route)'" == "cg, reduce(eliminate) precondition(jacobi)"
                assert e(fe_fallback) == 0
            }
        }
    }
}
display as result "END_FIXTURE|ols_schur_wide|rc=0"
