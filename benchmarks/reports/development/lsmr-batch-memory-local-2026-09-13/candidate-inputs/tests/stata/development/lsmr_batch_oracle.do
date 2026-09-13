version 18.0
clear all
set more off
set obs 4096
generate int g1 = 1 + mod(_n - 1, 20)
generate int g2 = 1 + mod(floor((_n - 1)/20) + 7*(_n - 1), 16)
generate int cl = 1 + mod(floor((_n - 1)/19), 97)
// Cross the wide-batch guard at both one and four installed workers.
local k = 65
local xs
forvalues j=1/`k' {
    generate double x`j' = sin(2*c(pi)*`j'*(_n-1)/_N) + cos(2*c(pi)*(`j'+97)*(_n-1)/_N)
    local xs `xs' x`j'
}
generate double off = 0.1*sin(_n/31)
generate double y = 1 + mod(floor(100*exp(0.2*x1-0.1*x2+0.03*g1+0.02*g2+off)),19)
generate double fw = 1 + mod(_n,7)
generate double pw = 0.2 + mod(_n,13)/3
generate double raw_w = 1

mata:
void batch_dummy_sandwich(string scalar xs, string scalar covariance, string scalar kind)
{
    g1 = st_data(., "g1")
    g2 = st_data(., "g2")
    G = J(rows(g1),35,0)
    for (j=1; j<=20; j++) G[.,j] = (g1 :== j)
    for (j=2; j<=16; j++) G[.,19+j] = (g2 :== j)
    X = (st_data(.,tokens(xs)),G)
    y = st_data(.,"y")
    mu = st_data(.,"mu_dummy")
    w = st_data(.,"raw_w")
    Hinv = invsym(quadcross(X,w:*mu,X))
    if (covariance == "cluster") {
        cl = st_data(.,"cl")
        sums = J(97,cols(X),0)
        for (i=1; i<=rows(X); i++) sums[cl[i],.] = sums[cl[i],.] + w[i]*(y[i]-mu[i])*X[i,.]
        meat = quadcross(sums,sums)
    }
    else {
        score_w = w:^2
        if (kind == "fw") score_w = w
        meat = quadcross(X,score_w:*(y-mu):^2,X)
    }
    // Replay the registered small-sample multiplier; this fixture independently
    // checks the full-dummy Hessian/meat algebra, not that multiplier's policy.
    V = Hinv*meat*Hinv*st_numscalar("correction")
    k = cols(tokens(xs))
    st_matrix("V_dummy",V[1..k,1..k])
}
end

foreach kind in unit fw pw {
    local weight ""
    replace raw_w = 1
    if "`kind'" != "unit" {
        local weight "[`kind'=`kind']"
        replace raw_w = `kind'
    }
    foreach t in 1 4 {
        local covariances "cluster"
        if "`kind'" == "unit" local covariances "robust cluster"
        foreach covariance of local covariances {
            local vce "robust"
            if "`covariance'" == "cluster" local vce "cluster cl"
            feppml y `xs' `weight', absorb(g1 g2) offset(off) ///
                solver(lsmr) vce(`vce') separation(none) keepsingletons ///
                threads(`t') mu(mu_native)
            matrix b_native = e(b)
            matrix V_native = e(V)
            scalar correction = e(feppml_inference)[3,1]
            assert e(converged) == 1
            assert e(feppml_certificate)[16,1] == 1
            assert e(sample) == 1
            quietly poisson y `xs' ibn.g1 ib1.g2 `weight', noconstant offset(off) nolog
            forvalues j=1/`k' {
                assert abs(_b[x`j']-b_native[1,`j']) <= 1e-7
            }
            predict double mu_dummy, n
            assert abs(mu_native-mu_dummy) <= 1e-7*(1+abs(mu_dummy))
            mata: batch_dummy_sandwich("`xs'","`covariance'","`kind'")
            matrix V_slopes = V_native[1..`k',1..`k']
            assert mreldif(V_slopes,V_dummy) <= 1e-7
            drop mu_native mu_dummy
        }
    }
}
display as result "END_FIXTURE|lsmr_batch_oracle|rc=0"
