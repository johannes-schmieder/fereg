version 18.0
clear all
set more off

* Independent explicit Poisson, Hessian and sandwich checks for categorical
* accepted-frame finalization, including FW multiplicity and PW cluster scores.
set obs 4800
generate long obs_id = _n
generate int g = 1 + mod(_n - 1, 8)
generate int h = 1 + mod(floor((_n - 1) / 9), 6)
generate int j = 1 + mod(floor((_n - 1) / 19), 5)
generate double z = sin(_n / 13) + mod(_n, 5) / 7
generate double w = cos(_n / 23) - mod(_n, 3) / 5
generate int cl = 1 + mod(floor((_n - 1) / 19), 97)
generate double x1 = sin(_n / 17) + mod(_n, 7) / 5
generate double x2 = cos(_n / 29) - mod(_n, 11) / 9
forvalues j = 3/8 {
    generate double x`j' = sin(_n / (13 + 7*`j')) + mod(_n, 7 + `j') / (5 + `j')
}
generate double off = 0.1 * sin(_n / 31)
generate double y = 1 + mod(floor(100 * exp(0.2*x1 - 0.1*x2 + 0.03*g + .07*g*z - .05*h*w + off)), 19)
generate double fw = 1 + mod(_n, 7)
generate double pw = 0.2 + mod(_n, 13) / 3
generate double raw_w = 1

mata:
void joint_column_sandwich(string scalar covariance, string scalar kind, real scalar dimensions)
{
    g = st_data(., "g")
    G = J(rows(g), 8, 0)
    for (j=1; j<=8; j++) G[.,j] = (g :== j)
    h = st_data(., "h")
    H = J(rows(h), 6, 0)
    for (j=1; j<=6; j++) H[.,j] = (h :== j)
    X = (st_data(., ("x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8")), G, H[.,2..6])
    if (dimensions == 3) {
        j = st_data(., "j")
        Jfe = J(rows(j), 5, 0)
        for (l=1; l<=5; l++) Jfe[.,l] = (j :== l)
        X = (X, Jfe[.,2..5])
    }
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

foreach solver in lsmr map auto {
foreach dimensions in 2 3 {
local effects "g h"
local dummies "ibn.g i.h"
if `dimensions' == 3 {
    local effects "g h j"
    local dummies "ibn.g i.h i.j"
}
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
        feppml y x1 x2 x3 x4 x5 x6 x7 x8 `weight', absorb(`effects') offset(off) ///
            vce(`vce') separation(none) keepsingletons threads(`threads') memory(fast) solver(`solver') mu(mu_native) eta(eta_native) xb(xb_native) d(d_native) residuals(res_native)
        local selected "`solver'"
        if "`solver'" == "auto" local selected "map"
        assert "`e(feppml_solver_selected)'" == "`selected'"
        assert abs(eta_native - ln(mu_native)) <= 1e-10
        assert abs(res_native - (y - mu_native)) <= 1e-10
        assert abs(eta_native - xb_native - d_native) <= 1e-9
        matrix b_native = e(b)
        matrix V_native = e(V)
        scalar correction = e(feppml_inference)[3,1]
        assert e(converged) == 1
        assert e(feppml_certificate)[16,1] == 1
        assert e(sample) == 1

        quietly poisson y x1 x2 x3 x4 x5 x6 x7 x8 `dummies' `weight', noconstant offset(off) nolog
        forvalues j = 1/8 {
            assert abs(_b[x`j'] - b_native[1,`j']) <= 1e-7
        }
        predict double mu_dummy, n
        assert abs(mu_native - mu_dummy) <= 1e-7 * (1 + abs(mu_dummy))

        mata: joint_column_sandwich("`covariance'", "`kind'", `dimensions')
        matrix V_slopes = V_native[1..8,1..8]
        assert mreldif(V_slopes, V_dummy) <= 1e-7
        drop mu_native mu_dummy eta_native xb_native d_native res_native
    }
}

}

}

}

* The automatic three-FE route must preserve the held estimate and output names
* when publication fails after the certified fit has completed.
quietly regress y x1 x2
matrix held_b = e(b)
matrix held_V = e(V)
generate byte held_sample = e(sample)
foreach stage in postplugin postpublic mutatesolver {
    capture noisily _fereg_estimate_ppml y x1 x2, absorb(g h j) offset(off) ///
        vce(robust) separation(none) keepsingletons threads(4) memory(fast) ///
        solver(auto) mu(__hybrid_mu) eta(__hybrid_eta) residual(__hybrid_resid) ///
        publicmode publiccmdline("feppml hybrid transaction") inject(`stage')
    local expected_rc = cond("`stage'" == "mutatesolver", 3499, 3498)
    assert _rc == `expected_rc'
    assert "`e(cmd)'" == "regress"
    assert mreldif(e(b), held_b) == 0
    assert mreldif(e(V), held_V) == 0
    assert e(sample) == held_sample
    foreach output in __hybrid_mu __hybrid_eta __hybrid_resid {
        confirm new variable `output'
    }
}

display as result "END_FIXTURE|feppml_categorical_final_oracle|rc=0"
