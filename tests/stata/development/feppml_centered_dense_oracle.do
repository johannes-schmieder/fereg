version 18.0
clear all
set more off

* Generated terms, an exact duplicate, interaction FE, changing public weights
* and requested outputs; independent explicit-dummy Poisson is the fit oracle.
set obs 6144
generate int g = 1 + mod(_n-1, 8)
generate byte h = 1 + mod(floor((_n-1)/8), 2)
egen int gh = group(g h)
generate byte f = 1 + mod(floor((_n-1)/16), 3)
generate double x = sin(_n/17) + mod(_n,7)/5
generate double x2 = cos(_n/29) - mod(_n,11)/9
generate double dup = 2*x2
generate double off = 0.1*sin(_n/31)
generate double y = 1 + mod(floor(100*exp(0.2*x - 0.1*x2 + 0.03*gh + off)), 19)
generate double fw = 1 + mod(_n,5)
generate double pw = 0.2 + mod(_n,13)/3
generate int cl = 1 + mod(floor((_n-1)/19), 31)

* Unsupported continuous-by-factor terms must still fail closed.
capture noisily feppml y c.x#i.f, absorb(g#h) separation(none)
assert _rc == 498

foreach threads in 1 4 {
    foreach kind in unit fw pw {
        local weight ""
        if "`kind'" != "unit" local weight "[`kind'=`kind']"
        local vce = cond("`kind'" == "pw", "cluster cl", "robust")
        feppml y c.x i.f c.x2 dup `weight', absorb(saved_g=g#h) ///
            offset(off) vce(`vce') separation(none) keepsingletons ///
            memory(fast) threads(`threads') mu(mu_native) eta(eta_native) ///
            xb(xb_native) d(d_native) residuals(r_native)
        assert e(converged) == 1
        assert e(sample) == 1
        assert e(feppml_certificate)[16,1] == 1
        scalar ll_native = e(ll)
        assert abs(mu_native-exp(eta_native)) <= 1e-10*(1+mu_native)
        assert abs(eta_native-xb_native-d_native) <= 1e-10
        assert abs(saved_g-d_native) <= 1e-10
        assert abs(r_native-(y-mu_native)) <= 1e-10

        quietly poisson y c.x i.f c.x2 dup ibn.gh `weight', ///
            noconstant offset(off) nolog
        predict double mu_dummy, n
        assert abs(mu_native-mu_dummy) <= 1e-7*(1+abs(mu_dummy))
        * Stata poisson reports raw-PW likelihood; ADR 0016 uses normalized q.
        * Replay the oracle mean with the declared objective, independently.
        generate double q_oracle = 1
        if "`kind'" == "fw" replace q_oracle = fw
        if "`kind'" == "pw" {
            quietly summarize pw, meanonly
            replace q_oracle = pw/r(mean)
        }
        generate double ll_term = q_oracle*(y*ln(mu_dummy)-mu_dummy-lngamma(y+1))
        quietly summarize ll_term, meanonly
        assert abs(ll_native-r(sum)) <= 1e-8*(1+abs(r(sum)))
        drop saved_g mu_native eta_native xb_native d_native r_native mu_dummy q_oracle ll_term
    }
}

display as result "END_FIXTURE|feppml_centered_dense_oracle|rc=0"
