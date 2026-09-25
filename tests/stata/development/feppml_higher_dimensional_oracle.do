version 18.0
clear all
set more off
set obs 960
generate long pair = floor((_n-1)/2)
generate int g = 1 + mod(pair, 8)
generate int h = 1 + mod(floor(pair/9), 6)
generate int j = 1 + mod(floor(pair/13), 5)
generate int l = 1 + mod(pair+floor(pair/7), 4)
generate int m = 1 + mod(floor(pair/7), 3)
generate int cl = 1 + mod(floor((_n-1)/19), 23)
generate double x1 = sin(pair/17) + mod(pair,7)/5
generate double x2 = cos(pair/29) - mod(pair,11)/9
generate double off = .1*sin(pair/31)
generate double q = 1
generate double truth_mu = .
generate double y = .

mata:
void higher_dimensional_truth_vce(real scalar dimensions, string scalar kind, string scalar covariance)
{
    X = st_data(., ("x1", "x2"))
    names = ("g", "h", "j", "l", "m")
    levels = (8, 6, 5, 4, 3)
    for (d=1; d<=dimensions; d++) {
        codes = st_data(., names[d])
        for (v=1+(d>1); v<=levels[d]; v++) X = (X, codes:==v)
    }
    w = st_data(., "q"); mu = st_data(., "truth_mu"); y = st_data(., "y")
    bread = invsym(quadcross(X,w:*mu,X))
    if (covariance == "cluster") {
        cl = st_data(., "cl")
        sums = J(23,cols(X),0)
        for (i=1; i<=rows(X); i++) sums[cl[i],.] = sums[cl[i],.] + w[i]*(y[i]-mu[i])*X[i,.]
        meat = quadcross(sums,sums)
    }
    else {
        score_w = w:^2
        if (kind == "fw") score_w = w
        meat = quadcross(X,score_w:*(y-mu):^2,X)
    }
    V = bread*meat*bread*st_numscalar("correction")
    st_matrix("V_truth",V[1..2,1..2])
}
end

foreach dimensions in 4 5 {
    local effects g h j l
    if `dimensions'==5 local effects `effects' m
    replace truth_mu = exp(.2*x1-.1*x2+.03*g+.02*h-.01*j+.015*l+off+(`dimensions'==5)*.02*m)
    foreach kind in unit fw pw {
        local weight ""
        local threads 1
        replace q = 1
        if "`kind'"=="fw" replace q = 1+mod(_n,7)
        if "`kind'"=="pw" replace q = .2+mod(_n,13)/3
        if "`kind'"!="unit" {
            local weight "[`kind'=q]"
            local threads 4
        }
        * Pair scores cancel exactly in mathematics. The means and finite
        * coefficients are known independently of either numerical estimator.
        replace y = truth_mu + cond(mod(_n,2)==1,.05*sin(pair*.71)/q,-.05*sin(pair*.71)/q)
        foreach memory in fast low {
            foreach covariance in robust cluster {
                local vce robust
                if "`covariance'"=="cluster" local vce cluster cl
                feppml y x1 x2 `weight', absorb(`effects') offset(off) ///
                    vce(`vce') separation(none) keepsingletons threads(`threads') ///
                    memory(`memory') poolsize(1) mu(mu_native)
                assert e(converged)==1
                assert e(feppml_certificate)[16,1]==1
                assert e(sample)==1
                assert abs(mu_native-truth_mu)<=1e-8*(1+truth_mu)
                matrix b=e(b)
                assert abs(b[1,1]-.2)<1e-9
                assert abs(b[1,2]+.1)<1e-9
                scalar correction=e(feppml_inference)[3,1]
                matrix V_native=e(V)
                matrix V_finite=V_native[1..2,1..2]
                mata: higher_dimensional_truth_vce(`dimensions',"`kind'","`covariance'")
                assert mreldif(V_finite,V_truth)<1e-8
                drop mu_native
            }
        }
    }
}
display as result "END_FIXTURE|feppml_higher_dimensional_oracle|rc=0"
