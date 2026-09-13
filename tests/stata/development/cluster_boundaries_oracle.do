version 18.0
clear all
set more off

* Independent full-dummy Poisson solve and sandwich, including current-weight
* Hessian, FW multiplicity, PW squared scores and a nonnested cluster partition.
set obs 65536
generate long obs_id = _n
generate int g = 1 + mod(_n - 1, 64)
generate byte ga = 1 + floor((g-1)/8)
generate byte gb = 1 + mod(g-1,8)
generate int cl = cond(mod(_n,10)!=0, 1, 2 + mod(floor(_n/10),96))
generate byte ca = 1 + floor((cl-1)/10)
generate byte cb = 1 + mod(cl-1,10)
generate double x1 = sin(_n / 17) + mod(_n, 7) / 5
generate double x2 = cos(_n / 29) - mod(_n, 11) / 9
forvalues j = 3/8 {
    generate double x`j' = 2^(`j'-4) * (cos(_n / (13+`j')) + 0.25*g)
}
generate double off = 0.1 * sin(_n / 31)
generate double y = 1 + mod(floor(100 * exp(0.2*x1 - 0.1*x2 + 0.03*g + off)), 19)
generate double fw = 1 + mod(_n, 7)
generate double pw = 0.2 + mod(_n, 13) / 3
generate double raw_w = 1

mata:
void one_fe_dummy_sandwich(string scalar covariance, string scalar kind)
{
    g = st_data(., "g")
    G = J(rows(g), 64, 0)
    for (j=1; j<=64; j++) G[.,j] = (g :== j)
    X = (st_data(., ("x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8")), G)
    y = st_data(., "y")
    mu = st_data(., "mu_dummy")
    w = st_data(., "raw_w")
    Hinv = invsym(quadcross(X, w:*mu, X))
    if (covariance == "cluster") {
        cl = st_data(., "cl")
        sums = J(97, cols(X), 0)
        for (i=1; i<=rows(X); i++) sums[cl[i],.] = sums[cl[i],.] + w[i]*(y[i]-mu[i])*X[i,.]
        meat = quadcross(sums, sums)
    }
    else {
        score_w = w:^2
        if (kind == "fw") score_w = w
        meat = quadcross(X, score_w:*(y-mu):^2, X)
    }
    V = Hinv * meat * Hinv * st_numscalar("correction")
    st_matrix("V_dummy", V[1..8,1..8])
}
end

foreach kind in unit fw pw {
    local weight ""
    replace raw_w = 1
    if "`kind'" != "unit" {
        local weight "[`kind'=`kind']"
        replace raw_w = `kind'
    }
    local covariance = cond("`kind'" == "pw", "cluster", "robust")
    foreach covariance in `covariance' {
        local vce "robust"
        if "`covariance'" == "cluster" local vce "cluster ca#cb"
        feppml y x1-x8 `weight', absorb(ga#gb) offset(off) ///
            vce(`vce') separation(none) keepsingletons threads(4) mu(mu_native)
        matrix b_native = e(b)
        matrix V_native = e(V)
        scalar correction = e(feppml_inference)[3,1]
        assert e(converged) == 1
        assert e(feppml_certificate)[16,1] == 1
        assert e(sample) == 1

        quietly poisson y x1-x8 ibn.g `weight', noconstant offset(off) nolog
        forvalues j = 1/8 {
            assert abs(_b[x`j'] - b_native[1,`j']) <= 1e-7
        }
        predict double mu_dummy, n
        assert abs(mu_native - mu_dummy) <= 1e-7 * (1 + abs(mu_dummy))

        mata: one_fe_dummy_sandwich("`covariance'", "`kind'")
        matrix V_slopes = V_native[1..8,1..8]
        assert mreldif(V_slopes, V_dummy) <= 1e-7
        drop mu_native mu_dummy
    }
}



clear all
set seed 652713
set obs 65536
generate long row = _n
generate byte ga = mod(_n,6)
generate byte gb = mod(floor((_n-1)/6),5)
generate byte g = 5*ga + gb
generate byte ca = cond(mod(_n,10)!=0, 0, 1 + mod(floor(_n/10),16))
generate byte cb = cond(mod(_n,10)!=0, 0, mod(floor(_n/160),19))
generate double w = 1 + mod(_n,7)
forvalues j=1/10 {
    generate double x`j' = rnormal()
}
generate double y = 0.2*x1 - 0.1*x3 + ga/10 + gb/20 + 0.2*rnormal()
replace x10 = .z in 5
replace cb = .a in 7
generate int cl = 19*ca + cb
generate str2 ca_string = string(ca)
foreach rankcase in full collinear {
    if "`rankcase'" == "collinear" replace x2 = x1
    foreach kind in unit aw fw pw {
        local weight ""
        if "`kind'" != "unit" local weight "[`kind'=w]"
        quietly regress y x1-x10 i.g `weight', vce(cluster cl)
        matrix oracle_b = e(b)
        matrix oracle_v = e(V)
        generate byte expected = e(sample)
        foreach absorption in g ga#gb {
            quietly fereg y x1-x10 `weight', absorb(`absorption') vce(cluster ca_string#cb) threads(4)
            assert e(sample) == expected
            assert row == _n
            forvalues j=1/10 {
                assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-8
                forvalues k=1/10 {
                    assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
                }
            }
            if "`rankcase'" == "full" assert e(rank) == 10
            if "`rankcase'" == "collinear" assert e(rank) == 9
        }
        drop expected
    }
}
display as result "END_FIXTURE|cluster_boundaries_oracle|rc=0"
