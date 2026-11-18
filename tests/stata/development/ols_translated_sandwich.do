version 18.0
clear all
set more off
set obs 504
generate byte replicate = mod(_n-1,2)
generate byte g = mod(floor((_n-1)/2),7)
generate byte h = mod(floor((_n-1)/14),6)
generate byte x0 = mod(floor((_n-1)/84),3)
generate byte z0 = floor((_n-1)/252)
generate byte cl = 1+mod(3*replicate+g+2*h+x0+2*z0,11)
generate double w = 1+mod(g+2*h+x0,3)
generate double x = x0 + 2^24
generate double z = z0 - 2^22
generate double noise = (replicate-.5)*(1+mod(x0+h,2))
generate double y = .375*x0-.25*z0+g/2-h/4+noise

* Fit explicit reference dummies in well-conditioned coordinates. The reported
* constant averages their coefficients over the estimation sample, then absorbs
* the two location shifts. Map the full reference covariance, including all
* slope/constant covariances, to those three reported coefficients.
quietly tab g, generate(dg)
quietly tab h, generate(dh)
foreach effects in none gh {
    local dummies ""
    local absorb ""
    if "`effects'"=="gh" {
        local dummies "dg2-dg7 dh2-dh6"
        local absorb "absorb(g h)"
    }
    foreach kind in unit aw fw pw {
        local weight ""
        local mean_weight ""
        if "`kind'"!="unit" {
            local weight "[`kind'=w]"
            local mean_weight "[aw=w]"
        }
        foreach vce in "robust" "cluster cl" {
            quietly regress y x0 z0 `dummies' `weight', vce(`vce')
            matrix T = J(3,colsof(e(b)),0)
            matrix T[1,1] = 1
            matrix T[2,2] = 1
            matrix T[3,1] = -(2^24)
            matrix T[3,2] = 2^22
            matrix T[3,colsof(T)] = 1
            if "`effects'"=="gh" {
                unab indicators : `dummies'
                foreach d of local indicators {
                    quietly summarize `d' `mean_weight', meanonly
                    matrix T[3,colnumb(e(b),"`d'")] = r(mean)
                }
            }
            matrix refV = T * e(V) * T'
            matrix refb = e(b) * T'
            foreach mode in fast low {
                quietly fereg y x z `weight', `absorb' vce(`vce') memory(`mode') threads(2) residuals(u)
                assert mreldif(e(b),refb)<1e-8
                forvalues j=1/3 {
                    forvalues k=1/3 {
                        * Scale each error by its two marginal standard errors.
                        assert abs(e(V)[`j',`k']-refV[`j',`k']) < 5e-9*sqrt(refV[`j',`j']*refV[`k',`k'])
                    }
                }
                if "`effects'"=="gh" assert abs(u-noise)<1e-8
                drop u
            }
        }
    }
}

* Each cluster has the same x values and a constant residual of +1 or -1.
* Its slope score vanishes, but its intercept influence is +/- 1/6. This
* isolates the 1/sum(w) term even at a location much larger than the spread.
clear
set obs 48
generate byte cl = floor((_n-1)/8)
generate byte x0 = mod(_n-1,2)
generate double x = x0 + 2^24
generate double y = .5*x0 + 2*mod(cl,2)-1
foreach mode in fast low {
    quietly fereg y x, vce(cluster cl) memory(`mode')
    assert abs(_b[x]-.5)<1e-10 & _se[x]<1e-12
    assert abs(e(V)[2,2]-(47/46)/5)<1e-8
}
display as result "END_FIXTURE|ols_translated_sandwich|rc=0"
