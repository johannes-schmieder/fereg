version 18
clear all
set more off

* The finite predictor is .5*x+.25*w. The two reported slopes represent
* exactly the same model in coordinates (x, x+2^-18*w).
set obs 256
generate byte cell = mod(floor((_n-1)/2),4)
generate byte h = mod(floor((_n-1)/8),4)
generate byte g = mod(floor((_n-1)/32),4)
generate byte shock = cond(mod(_n,2),-1,1)
generate int cl = 1+mod(2*cell+(shock==1)+h,7)
generate double x = cond(cell<2,2*cell-1,0)
generate double w = cond(cell>=2,2*cell-5,0)
generate double t = 2^-18*w
generate double z = x+t
generate double q = .
generate double mean = 3+g/4+h/8+x/2+w/4
generate double y = .

foreach vce in iid clustered {
    if "`vce'"=="iid" {
        quietly replace q=cond(w==0,1,2^36)
        quietly replace y=mean+shock/2
        local options "vce(unadjusted)"
        local regopt
        local weights "[aw=q]"
    }
    else {
        quietly replace q=cond(w==0,1,2)
        quietly replace y=mean+shock*cond(w==0,0.5,2^-18)
        local options "vce(cluster cl)"
        local regopt "vce(cluster cl)"
        local weights "[fw=q]"
    }
    * Explicit-dummy OLS in the uncorrelated (x,t) directions supplies
    * every target independently, including the Stata joint-test rank.
    quietly regress y x t i.g i.h `weights', `regopt'
    quietly test x t
    scalar joint_F=r(F)
    assert r(df)==2
    quietly lincom x-t
    scalar orig_b=r(estimate)
    scalar orig_se=r(se)
    scalar second_se=_se[t]
    scalar common_se=_se[x]
    foreach memory in fast low {
        quietly fereg y x z `weights', absorb(g h) `options' memory(`memory') threads(1) residuals(r)
        assert e(df_m)==2 & reldif(_b[x],orig_b)<1e-7 & reldif(_b[z],65536)<1e-7
        assert reldif(_se[x],orig_se)<1e-6 & reldif(_se[z],second_se)<1e-6
        assert reldif(e(F),joint_F)<1e-6
        quietly test x z
        assert r(df)==2 & reldif(r(F),joint_F)<1e-6
        quietly lincom x+z
        assert abs(r(estimate)-.5)<1e-8 & reldif(r(se),common_se)<1e-6
        quietly predict double fitted, xbd
        assert abs(fitted-mean)<1e-8 & abs(r-(y-mean))<1e-8
        drop r fitted
    }
}

* The separately included w makes z exactly aliased with x and w. Recoding
* z makes its working column a copy of w, which can reverse their pivot order.
* The original robust fit omits z and keeps both independently named slopes.
quietly fereg y x z w [fw=q], absorb(g h) vce(robust) residuals(r)
local columns : colnames e(b)
assert "`columns'"=="x o.z w _cons" & e(df_m)==2
assert abs(_b[x]-.5)<1e-8 & abs(_b[w]-.25)<1e-8 & _b[z]==0 & _se[z]==0
assert abs(r-(y-mean))<1e-8

* Two weak pairs share exactly the same difference. Dropping x leaves a
* second ill-conditioned pair (u,k). The centered x and w are orthogonal
* to k and the constant, so V[u,_cons] is exactly zero on this pivot.
clear
set obs 32
generate byte cell = floor((_n-1)/8)
generate byte x = cond(cell<2,2*cell-1,0)
generate byte w = cond(cell>=2,2*cell-5,0)
generate byte k = mod(floor((_n-1)/2),2)
generate byte shock = cond(mod(_n,2),-1,1)
generate double z = x+2^-15*w
generate double u = .5*k-3*2^-15*w
generate double mean = 2+x/2+w/4+k/2
generate double y = mean+shock*(1+abs(x)+k)*(1+w/4)/4
foreach memory in fast low {
    quietly regress y x w k
    matrix previous_b=e(b)
    matrix previous_V=e(V)
    capture noisily fereg y u z x k, vce(unadjusted) threads(1) memory(`memory') residuals(r)
    local rc=_rc
    if `rc' {
        assert `rc'==430
        mata: assert(st_matrix("e(b)")==st_matrix("previous_b"))
        mata: assert(st_matrix("e(V)")==st_matrix("previous_V"))
        confirm new variable r
    }
    else {
        local names : colnames e(b)
        assert "`names'"=="u z o.x k _cons"
        assert abs(_b[u]+2730.5)<1e-7 & abs(_b[z]-.5)<1e-9 & abs(_b[k]-1365.75)<1e-7
        matrix V=e(V)
        assert abs(V[1,5])<=1e-10
        assert abs(y-r-mean)<1e-8
        drop r
    }
}
display as result "END_FIXTURE|slope_recovery_ols_inference|rc=0"
