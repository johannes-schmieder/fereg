version 18.0
clear all
set more off
set seed 739391
set obs 16384
forvalues j = 1/4 {
    generate double x`j' = rnormal()
}
generate byte q = mod(_n, 7)
generate double y = 2
generate double fw = 1 + mod(_n, 5)
generate double pw = .25 + mod(_n, 13)/8

* A full-rank design with a constant outcome has the unique finite optimum
* beta=(0,...,0,ln(2)), regardless of the positive observation weights.
foreach kind in unit fw pw {
    local weight ""
    if "`kind'" != "unit" local weight "[`kind'=`kind']"
    foreach mode in fast low {
        foreach t in 1 4 {
            foreach covariance in robust cluster {
                local vce "robust"
                if "`covariance'"=="cluster" local vce "cluster q"
                quietly feppml y x1-x4 i.q `weight', memory(`mode') ///
                    poolsize(1) threads(`t') tolerance(1e-12) ///
                    vce(`vce') separation(none) mu(mu_native)
                assert e(converged)==1
                assert e(sample)==1
                assert e(feppml_certificate)[16,1]==1
                forvalues j = 1/4 {
                    assert abs(_b[x`j'])<1e-13
                }
                forvalues j = 1/6 {
                    assert abs(_b[`j'.q])<1e-13
                }
                assert abs(_b[_cons]-ln(2))<1e-13
                assert abs(mu_native-2)<2e-13
                drop mu_native
            }
        }
    }
}
display as result "END_FIXTURE|feppml_no_fe_strict_oracle|rc=0"
