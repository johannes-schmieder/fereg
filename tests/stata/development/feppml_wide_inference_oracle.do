version 18.0
clear all
set more off

* Independent full-dummy Poisson solve and sandwich, including current-weight
* Hessian, FW multiplicity, PW squared scores and a nonnested cluster partition.
set obs 8192
generate long obs_id = _n
generate int g = 1 + mod(_n - 1, 16)
generate int cl = 1 + mod(floor((_n - 1) / 19), 97)
generate double x1 = sin(_n / 17) + mod(_n, 7) / 5
generate double x2 = cos(_n / 29) - mod(_n, 11) / 9
forvalues j = 3/24 {
    generate double x`j' = sin(_n/(3+1.7*`j')) + cos(_n/(11+0.37*`j')) + mod(_n,`j'+3)/(`j'+3)
}
generate double off = 0.1 * sin(_n / 31)
generate double y = 1 + mod(floor(100 * exp(0.2*x1 - 0.1*x2 + 0.03*g + off)), 19)
generate double fw = 1 + mod(_n, 7)
generate double pw = 0.2 + mod(_n, 13) / 3
generate double raw_w = 1

mata:
void wide_inference_dummy_sandwich(string scalar covariance, string scalar kind)
{
    g = st_data(., "g")
    G = J(rows(g), 16, 0)
    for (j=1; j<=16; j++) G[.,j] = (g :== j)
    X = (st_data(., tokens(st_local("regressors"))), G)
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
    st_matrix("V_dummy", V[1..24,1..24])
}
end

unab regressors: x1-x24
foreach threads in 1 4 {
foreach kind in unit fw pw {
    local weight ""
    replace raw_w = 1
    if "`kind'" != "unit" {
        local weight "[`kind'=`kind']"
        replace raw_w = `kind'
    }
    foreach covariance in robust cluster {
        local vce "robust"
        if "`covariance'" == "cluster" local vce "cluster cl"
        feppml y `regressors' `weight', absorb(g) offset(off) ///
            vce(`vce') separation(none) keepsingletons threads(`threads') mu(mu_native)
        matrix b_native = e(b)
        matrix V_native = e(V)
        scalar correction = e(feppml_inference)[3,1]
        assert e(converged) == 1
        assert e(feppml_certificate)[16,1] == 1
        assert e(sample) == 1

        quietly poisson y `regressors' ibn.g `weight', noconstant offset(off) nolog
        forvalues j = 1/24 {
            assert abs(_b[x`j'] - b_native[1,`j']) <= 1e-7 * (1 + abs(_b[x`j']))
        }
        predict double mu_dummy, n
        assert abs(mu_native - mu_dummy) <= 1e-7 * (1 + abs(mu_dummy))

        mata: wide_inference_dummy_sandwich("`covariance'", "`kind'")
        matrix V_slopes = V_native[1..24,1..24]
        assert mreldif(V_slopes, V_dummy) <= 1e-7
        drop mu_native mu_dummy
    }
}

}

display as result "END_FIXTURE|feppml_wide_inference_oracle|rc=0"
