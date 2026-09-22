version 18.0
clear all
set more off

* Explicit LSMR reaches canonical small-batch checkpoints at the row cutoff.
* Independent dummy Poisson and current-Hessian sandwiches check changing
* weights, while the separate solver trace records actual RHS admissions.
set obs 16384
generate int g1 = 1 + mod(_n-1, 32)
generate int g2 = 1 + mod(floor((_n-1)/7), 19)
generate int cl = 1 + mod(floor((_n-1)/19), 97)
forvalues j=1/12 {
    generate double x`j' = sin(_n/(13+`j')) + cos(_n/(23+2*`j'))
}
generate double off = 0.1*sin(_n/31)
generate double y = 1 + mod(floor(100*exp(.2*x1-.1*x2+.03*g1-.02*g2+off)),19)
generate double fw = 1 + mod(_n,7)
generate double pw = .2 + mod(_n,13)/3
generate double raw_w = 1

mata:
void narrow_checkpoint_sandwich(real scalar k, string scalar kind)
{
    names = J(1,k,"")
    for (j=1; j<=k; j++) names[j] = "x"+strofreal(j)
    X = st_data(., names)
    for (d=1; d<=2; d++) {
        g = st_data(., "g"+strofreal(d))
        first = (d==1 ? 1 : 2)
        for (j=first; j<=max(g); j++) X = (X, g:==j)
    }
    y = st_data(., "y")
    mu = st_data(., "mu_dummy")
    w = st_data(., "raw_w")
    Hinv = invsym(quadcross(X, w:*mu, X))
    if (kind=="pw") {
        cl = st_data(., "cl")
        sums = J(97,cols(X),0)
        for (i=1; i<=rows(X); i++) sums[cl[i],.] = sums[cl[i],.] + w[i]*(y[i]-mu[i])*X[i,.]
        meat = quadcross(sums,sums)
    }
    else {
        score_w = w:^2
        if (kind=="fw") score_w = w
        meat = quadcross(X, score_w:*(y-mu):^2, X)
    }
    V = Hinv*meat*Hinv*st_numscalar("correction")
    st_matrix("V_dummy",V[1..k,1..k])
}
end

foreach k in 1 2 6 12 {
    foreach kind in unit fw pw {
        local weight ""
        replace raw_w=1
        if "`kind'"!="unit" {
            local weight "[`kind'=`kind']"
            replace raw_w=`kind'
        }
        local vce robust
        if "`kind'"=="pw" local vce cluster cl
        feppml y x1-x`k' `weight', absorb(g1 g2) offset(off) ///
            vce(`vce') separation(none) keepsingletons solver(lsmr) threads(4) mu(mu_native)
        matrix b_native=e(b)
        matrix V_native=e(V)
        scalar correction=e(feppml_inference)[3,1]
        assert e(converged)==1 & e(feppml_certificate)[16,1]==1
        assert e(sample)==1
        quietly poisson y x1-x`k' ibn.g1 i.g2 `weight', noconstant offset(off) ///
            nolog tolerance(1e-12) ltolerance(1e-12) nrtolerance(1e-12)
        forvalues j=1/`k' {
            assert abs(_b[x`j']-b_native[1,`j'])<=1e-10+1e-7*abs(_b[x`j'])
        }
        predict double mu_dummy,n
        assert abs(mu_native-mu_dummy)<=1e-7*(1+abs(mu_dummy))
        mata: narrow_checkpoint_sandwich(`k',"`kind'")
        mata: assert(all(abs(st_matrix("V_native")[1..`k',1..`k']-st_matrix("V_dummy")) :<= 1e-10:+1e-6:*abs(st_matrix("V_dummy"))))
        feppml y x1-x`k' `weight', absorb(g1 g2) offset(off) ///
            vce(`vce') separation(none) keepsingletons solver(lsmr) threads(1)
        assert mreldif(e(b),b_native)<=1e-10
        assert mreldif(e(V),V_native)<=1e-10
        drop mu_native mu_dummy
    }
}
display as result "END_FIXTURE|feppml_narrow_checkpoint_schur|rc=0"
