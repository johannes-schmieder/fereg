version 18.0
clear all
set more off
set obs 98304
generate long row = _n
generate byte g = mod(_n,8)
generate byte cluster = mod(_n,17)
generate double w = 1 + mod(_n,5)
forvalues j=1/50 {
    generate double x`j' = sin(_n/(13+`j')) + cos(_n/(29+2*`j'))
}
generate double y = 0.2*x1 - 0.1*x3 + g/10 + 0.1*sin(_n/7)
quietly replace x50 = .z in 5
foreach rankcase in full collinear {
    if "`rankcase'" == "collinear" quietly replace x2 = x1
    foreach kind in unit aw fw pw {
        local weight ""
        if "`kind'" != "unit" local weight "[`kind'=w]"
        foreach covariance in cluster {
            local vce "robust"
            if "`covariance'" == "cluster" local vce "cluster cluster"
            quietly regress y x1-x50 i.g `weight', vce(`vce')
            matrix oracle_b = e(b)
            matrix oracle_v = e(V)
            generate byte expected = e(sample)
            quietly fereg y x1-x50 `weight', absorb(g) vce(`vce') threads(4) memory(low)
            assert e(sample) == expected
            forvalues j=1/50 {
                assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-8
                forvalues k=1/50 {
                    assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
                }
            }
            assert row == _n
            if "`rankcase'" == "full" assert e(rank) == 50
            if "`rankcase'" == "collinear" assert e(rank) == 49
            drop expected
        }
    }
}
display as result "END_FIXTURE|ols_cluster_low_memory|rc=0"
