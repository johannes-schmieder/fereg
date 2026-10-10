version 18.0
clear all
set more off

* Explicit-design Poisson and sandwich oracle for requested/effective teams,
* including saved-effect recovery and the excluded low-memory route.
set obs 4800
generate int g = 1 + mod(_n-1, 8)
generate int h = 1 + mod(floor((_n-1)/9), 6)
generate int j = 1 + mod(floor((_n-1)/19), 5)
generate int cl = 1 + mod(floor((_n-1)/19), 97)
generate double x1 = sin(_n/17) + mod(_n,7)/5
generate double x2 = cos(_n/29) - mod(_n,11)/9
generate double x3 = sin(_n/37) + mod(_n,13)/8
generate double off = .1*sin(_n/31)
generate double y = 1 + mod(floor(100*exp(.2*x1-.1*x2+.04*x3+.03*g+.02*h+off)),19)
generate double fw = 1 + mod(_n,7)
generate double pw = .2 + mod(_n,13)/3
generate double raw_w = 1

mata:
void ceiling_sandwich(string scalar covariance, string scalar kind)
{
    g = st_data(., "g")
    G = J(rows(g), 8, 0)
    for (l=1; l<=8; l++) G[.,l] = (g :== l)
    h = st_data(., "h")
    H = J(rows(h), 5, 0)
    for (l=2; l<=6; l++) H[.,l-1] = (h :== l)
    j = st_data(., "j")
    Jfe = J(rows(j), 4, 0)
    for (l=2; l<=5; l++) Jfe[.,l-1] = (j :== l)
    X = (st_data(., ("x1", "x2", "x3")), G, H, Jfe)
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
        sw = w:^2
        if (kind == "fw") sw = w
        meat = quadcross(X, sw:*(y-mu):^2, X)
    }
    V = Hinv * meat * Hinv * st_numscalar("correction")
    st_matrix("V_dummy", V[1..3,1..3])
}
end

foreach memory in fast low {
foreach kind in fw pw {
foreach covariance in robust cluster {
    replace raw_w = `kind'
    local vce "robust"
    if "`covariance'" == "cluster" local vce "cluster cl"
    feppml y x1 x2 x3 [`kind'=`kind'], absorb(ge=g he=h je=j) offset(off) ///
        vce(`vce') separation(none) keepsingletons threads(16) memory(`memory') ///
        solver(auto) mu(mu_native) eta(eta_native) xb(xb_native) d(d_native) residuals(res_native)
    assert e(feppml_requested_threads) == 16
    assert e(feppml_threads) == 16
    assert e(converged) == 1 & e(sample) == 1
    assert e(feppml_certificate)[16,1] == 1
    assert abs(eta_native-ln(mu_native)) <= 1e-10
    assert abs(res_native-(y-mu_native)) <= 1e-10
    assert abs(eta_native-xb_native-d_native) <= 1e-9
    assert abs(d_native-ge-he-je) <= 1e-9
    matrix b_native = e(b)
    matrix V_native = e(V)
    scalar correction = e(feppml_inference)[3,1]
    quietly poisson y x1 x2 x3 ibn.g i.h i.j [`kind'=`kind'], noconstant offset(off) nolog
    forvalues l=1/3 {
        assert abs(_b[x`l']-b_native[1,`l']) <= 1e-7
    }
    predict double mu_dummy, n
    assert abs(mu_native-mu_dummy) <= 1e-7*(1+abs(mu_dummy))
    mata: ceiling_sandwich("`covariance'", "`kind'")
    matrix V_finite = V_native[1..3,1..3]
    assert mreldif(V_finite,V_dummy) <= 1e-7
    drop ge he je mu_native mu_dummy eta_native xb_native d_native res_native
}
}
}

* Repeated requests reuse the selected team without changing request identity.
foreach threads in 8 28 1 16 {
    feppml y x1 x2 x3, absorb(g h j) offset(off) vce(robust) ///
        separation(none) keepsingletons threads(`threads') memory(fast)
    assert e(feppml_requested_threads) == `threads'
    assert e(feppml_threads) == `threads'
    assert e(converged) == 1
}

quietly regress y x1 x2 x3
matrix held_b = e(b)
matrix held_V = e(V)
generate byte held_sample = e(sample)
capture noisily _fereg_estimate_ppml y x1 x2 x3, absorb(g h j) offset(off) ///
    vce(robust) separation(none) keepsingletons threads(16) memory(fast) ///
    solver(auto) mu(__ceiling_mu) eta(__ceiling_eta) residual(__ceiling_resid) ///
    publicmode publiccmdline("feppml ceiling transaction") inject(postplugin)
assert _rc == 3498
assert "`e(cmd)'" == "regress"
assert mreldif(e(b),held_b) == 0 & mreldif(e(V),held_V) == 0
assert e(sample) == held_sample
foreach output in __ceiling_mu __ceiling_eta __ceiling_resid {
    confirm new variable `output'
}

display as result "END_FIXTURE|feppml_thread_ceiling_oracle|rc=0"
