version 18.0
clear all
set more off
set seed 928174
set obs 32768
generate long row = _n
generate byte g = mod(_n,8)
generate byte c1 = mod(_n,17)
generate byte c2 = mod(floor((_n-1)/17),19)
generate int joint = c1 + 17*c2
generate double w = 1 + mod(_n,5)
forvalues j=1/10 {
    generate double x`j' = rnormal() + sin(_n/(13+`j'))
}
generate double y = 0.2*x1 - 0.1*x3 + g/10 + 0.4*sin(c1) + 0.3*cos(c2) + 0.2*rnormal()
quietly replace x10 = .z in 5
foreach rankcase in full collinear {
    if "`rankcase'" == "collinear" quietly replace x2 = x1
    foreach kind in unit aw fw pw {
        local weight ""
        if "`kind'" != "unit" local weight "[`kind'=w]"
        // Obtain design rank before VCE can reduce e(rank) to covariance rank.
        local rankweight "`weight'"
        if "`kind'" == "pw" local rankweight "[aw=w]"
        quietly regress y x1-x10 i.g `rankweight'
        scalar design_rank = e(rank)
        if "`rankcase'" == "full" assert design_rank == 18
        if "`rankcase'" == "collinear" assert design_rank == 17
        foreach cluster in c1 c2 joint {
            quietly regress y x1-x10 i.g `weight', vce(cluster `cluster')
            display "ORACLE_RANK|kind=`kind'|case=`rankcase'|cluster=`cluster'|design=" design_rank "|covariance=" e(rank)
            matrix v_`cluster' = e(V)
            scalar q_`cluster' = ((e(N)-1)/(e(N)-design_rank))*(e(N_clust)/(e(N_clust)-1))
            matrix v_`cluster' = v_`cluster'/q_`cluster'
        }
        matrix oracle_b = e(b)
        matrix oracle_v = q_c1*(v_c1 + v_c2 - v_joint)
        // With a reported intercept, the registered CGM policy restores the
        // independently eigen-clipped raw slope block after any full repair.
        mata: raw_slope = st_matrix("oracle_v")[1..10,1..10]
        mata: symeigensystem((raw_slope+raw_slope')/2, eigvec=., eigval=.)
        mata: st_numscalar("eigen_error", max(abs(raw_slope-eigvec*diag(eigval)*eigvec')))
        assert eigen_error < 1e-15
        mata: st_numscalar("oracle_min_raw", min(eigval))
        mata: st_matrix("oracle_slope", eigvec * diag(eigval :* (eigval :> 0)) * eigvec')
        display "ORACLE_PSD|kind=`kind'|case=`rankcase'|raw_min=" oracle_min_raw
        generate byte expected = e(sample)
        quietly fereg y x1-x10 `weight', absorb(g) vce(cluster c1 c2) threads(4) memory(low)
        assert e(sample) == expected
        assert e(df_r) == 16
        assert e(N_clust1) == 17
        assert e(N_clust2) == 19
        forvalues j=1/10 {
            assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-8
            forvalues k=1/10 {
                assert abs(e(V)[`j',`k']-oracle_slope[`j',`k']) < 1e-10
            }
        }
        assert row == _n
        if "`rankcase'" == "full" assert e(rank) == 10
        if "`rankcase'" == "collinear" assert e(rank) == 9
        drop expected
    }
}
display as result "END_FIXTURE|ols_cluster_batch|rc=0"
