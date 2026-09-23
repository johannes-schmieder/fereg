version 18.0
clear all
set more off
set seed 202609229
set obs 16416
generate long g1 = mod(_n-1,32)
generate long g2 = mod(17*(_n-1)+floor((_n-1)/32),19)
generate long g3 = mod(11*(_n-1)+floor((_n-1)/47),13)
forvalues j=1/17 {
    generate double x`j' = rnormal()
}
generate double y = .7*x1-.3*x2+g1/30+g2/40+g3/50+rnormal()
generate double w = .1*(1+mod(_n,17))
foreach weighted in 0 1 {
    local weight
    if `weighted' local weight [aw=w]
    quietly regress y x1-x17 i.g1 i.g2 i.g3 `weight', vce(robust)
    matrix oracle_b = e(b)
    matrix oracle_v = e(V)
    scalar oracle_rss = e(rss)
    scalar oracle_df = e(df_r)
    foreach t in 1 4 16 {
        quietly fereg y x1-x17 `weight', absorb(g1 g2 g3) ///
            solver(lsmr) threads(`t') vce(robust)
        assert e(fereg_certificate)[32,2] == 1
        assert e(df_r) == oracle_df
        forvalues j=1/17 {
            assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-8
            forvalues k=1/17 {
                assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
            }
        }
        assert abs(e(rss)-oracle_rss) <= 1e-9*oracle_rss
    }
}
display as result "END_FIXTURE|ols_three_fe_batch_oracle|rc=0"
