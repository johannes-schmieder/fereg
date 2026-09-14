version 18.0
clear all
set more off
set obs 16384
generate long obs_id = _n
generate int g1 = 1 + mod(_n-1,16)
generate int g2 = 1 + mod(floor((_n-1)/16),12)
generate int cl = 1 + mod(floor((_n-1)/19),97)
forvalues j=1/16 {
    generate double x`j' = sin(_n/(11+`j')) + cos(_n/(31+2*`j')) + .05*g1 + .02*g2
}
generate double off = .1*sin(_n/31)
generate double y = 1 + mod(floor(100*exp(.2*x1-.1*x2+.03*g1-.02*g2+off)),19)
generate double fw = 1 + mod(_n,7)
generate double pw = .2 + mod(_n,13)/3
generate double raw_w = 1

mata:
void two_fe_dummy_sandwich(string scalar covariance, string scalar kind)
{
    a = st_data(.,"g1")
    b = st_data(.,"g2")
    G = J(rows(a),27,0)
    for (j=1;j<=16;j++) G[.,j] = (a:==j)
    for (j=2;j<=12;j++) G[.,15+j] = (b:==j)
    X = J(rows(a),0,0)
    for (j=1;j<=16;j++) X = (X,st_data(.,"x"+strofreal(j)))
    X = (X,G)
    y = st_data(.,"y")
    mu = st_data(.,"mu_dummy")
    w = st_data(.,"raw_w")
    Hinv = invsym(quadcross(X,w:*mu,X))
    if (covariance=="cluster") {
        cl = st_data(.,"cl")
        sums = J(97,cols(X),0)
        for (i=1;i<=rows(X);i++) sums[cl[i],.] = sums[cl[i],.] + w[i]*(y[i]-mu[i])*X[i,.]
        meat = quadcross(sums,sums)
        correction = 97/96
    }
    else {
        score_w = w:^2
        effective_n = rows(X)
        if (kind=="fw") {
            score_w = w
            effective_n = sum(w)
        }
        meat = quadcross(X,score_w:*(y-mu):^2,X)
        correction = effective_n/(effective_n-1)
    }
    V = Hinv*meat*Hinv*correction
    st_matrix("V_dummy",V[1..16,1..16])
    st_numscalar("correction_dummy",correction)
}
end

foreach kind in unit fw pw {
    local weight ""
    quietly replace raw_w = 1
    if "`kind'" != "unit" {
        local weight "[`kind'=`kind']"
        quietly replace raw_w = `kind'
    }
    local covariance = cond("`kind'"=="pw","cluster","robust")
    local vce "robust"
    if "`covariance'"=="cluster" local vce "cluster cl"
    quietly poisson y x1-x16 ibn.g1 ib1.g2 `weight', noconstant offset(off) nolog
    matrix b_dummy = e(b)
    predict double mu_dummy, n
    mata: two_fe_dummy_sandwich("`covariance'","`kind'")
    foreach solver in auto map cmg {
        feppml y x1-x16 `weight', absorb(g1 g2) offset(off) vce(`vce') ///
            separation(none) keepsingletons threads(4) solver(`solver') mu(mu_native)
        assert e(converged)==1
        assert e(sample)==1
        assert e(feppml_certificate)[16,1]==1
        assert abs(e(feppml_inference)[3,1]-correction_dummy)<1e-14
        matrix b_native = e(b)
        matrix V_native = e(V)
        forvalues j=1/16 {
            assert abs(b_native[1,`j']-b_dummy[1,`j'])<=1e-7
        }
        assert abs(mu_native-mu_dummy)<=1e-7*(1+abs(mu_dummy))
        matrix V_slopes = V_native[1..16,1..16]
        assert mreldif(V_slopes,V_dummy)<=1e-7
        display as result "ORACLE_FIT|kind=`kind'|solver=`solver'|rc=0"
        drop mu_native
    }
    drop mu_dummy
}
display as result "END_FIXTURE|two_fe_schur_routes|rc=0"
