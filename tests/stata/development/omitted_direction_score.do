version 18
clear all
set more off
set obs 128
generate byte a=mod(_n-1,2)
generate byte b=mod(floor((_n-1)/2),2)
generate byte g=mod(floor((_n-1)/4),4)
generate byte h=mod(floor((_n-1)/16),2)
generate byte r=mod(floor((_n-1)/32),4)
generate double w=1+mod(g+h,3)
generate double mu=2^(b+mod(g,2)+h)
generate double level=3*b+mod(g,2)+h
generate double y=mu*(1+(r==1)-(r==0))
generate double yl=level+(r==1)-(r==0)
generate double z=b

* Each replicate quartet has the stated conditional mean, including its zero.
* Neither response contains separation, and w is constant within a cell.
foreach memory in fast low {
    quietly fereg yl a z [pw=w], absorb(g h) memory(`memory') residuals(res)
    assert abs(yl-res-level)<1e-8
    drop res
    quietly feppml y a z [pw=w], absorb(ref_g=g ref_h=h) memory(`memory') separation(none) mu(m)
    assert abs(m/mu-1)<1e-8
    scalar ref_a = _b[a]
    scalar ref_z = _b[z]
    scalar ref_sz = _se[z]
    quietly lincom a + z
    scalar ref_contrast = r(se)
    quietly test (a=0) (z=0)
    scalar ref_df = r(df)
    scalar ref_chi2 = r(chi2)
    drop m
    foreach kind in fe finite {
        if "`kind'"=="fe" quietly replace z=mod(g,2)+2^(-26)*b
        else quietly replace z=a+2^(-41)*b
        * Both recodings preserve b exactly in the stored data. The ordinary
        * two-slope covariance loses the identified contrast in raw units.
        quietly regress yl a
        capture noisily fereg yl a z [pw=w], absorb(g h) memory(`memory') residuals(res)
        assert _rc==430
        assert "`e(cmd)'"=="regress"
        capture confirm variable res, exact
        assert _rc==111
        if "`kind'"=="finite" {
            capture noisily feppml y a z [pw=w], absorb(g h) memory(`memory') separation(none) mu(m)
            assert _rc==430
            assert "`e(cmd)'"=="regress"
            capture confirm variable m, exact
            assert _rc==111
        }
        else {
            quietly feppml y a z [pw=w], absorb(raw_g=g raw_h=h) memory(`memory') ///
                separation(none) mu(m) eta(eta) xb(xb) d(d)
            assert e(N)==128 & _b[_cons]==0
            assert abs(_b[a]-ref_a)<1e-9 & abs(_b[z]*2^(-26)-ref_z)<1e-9
            assert abs(_se[z]*2^(-26)-ref_sz)<1e-10
            local unit = 2^(-26)
            quietly lincom a + `unit'*z
            assert abs(r(se)-ref_contrast)<1e-9
            quietly test (a=0) (z=0)
            assert r(df)==ref_df & reldif(r(chi2),ref_chi2)<1e-7
            predict double scored, xb
            predict double full, mu
            assert abs(scored-xb)<1e-8 & abs(full/mu-1)<1e-8
            assert abs(m/mu-1)<1e-8 & abs(eta-ln(mu))<1e-8
            assert abs(xb+d-ln(mu))<1e-8 & abs(raw_g+raw_h-d)<1e-8
            assert abs(raw_g+mod(g,2)*_b[z]-ref_g)<1e-8 & abs(raw_h-ref_h)<1e-8
            drop raw_g raw_h m eta xb d scored full
        }
    }
    drop ref_g ref_h

    * With a centered anchor the normalized OLS constant and its covariance
    * are representable, even for a much smaller within-group regressor.
    quietly replace z=b
    quietly fereg yl a z, absorb(ref_g=g ref_h=h) memory(`memory') residuals(res)
    scalar ref_cons = _b[_cons]
    scalar ref_scons = _se[_cons]
    scalar ref_z = _b[z]
    scalar ref_sz = _se[z]
    quietly lincom a+z
    scalar ref_contrast = r(se)
    quietly test (a=0) (z=0)
    scalar ref_df = r(df)
    scalar ref_F = r(F)
    drop res
    quietly replace z=mod(g,2)-.5 + 2^(-38)*b
    quietly fereg yl a z, absorb(raw_g=g raw_h=h) memory(`memory') residuals(res)
    assert abs(_b[_cons]-ref_cons)<1e-9 & abs(_se[_cons]-ref_scons)<1e-9
    assert abs(_b[z]*2^(-38)-ref_z)<1e-9 & abs(_se[z]*2^(-38)-ref_sz)<1e-9
    local unit = 2^(-38)
    quietly lincom a + `unit'*z
    assert abs(r(se)-ref_contrast)<1e-9
    quietly test (a=0) (z=0)
    assert r(df)==ref_df & reldif(r(F),ref_F)<1e-7
    predict double scored, xb
    predict double d, d
    predict double fitted, xbd
    assert abs(yl-res-level)<1e-8 & abs(fitted-level)<1e-8 & abs(scored+d-level)<1e-8
    assert abs(raw_g+(mod(g,2)-.5)*_b[z]-ref_g)<1e-8 & abs(raw_h-ref_h)<1e-8
    drop ref_g ref_h res raw_g raw_h scored d fitted

    * Even an exactly rescalable PPML slope and covariance do not guarantee
    * representable XB and D. Their sum loses the log mean at this depth.
    quietly replace z=mod(g,2) + 2^(-44)*b
    quietly regress yl a
    capture noisily feppml y a z [pw=w], absorb(g h) memory(`memory') separation(none) mu(m)
    assert _rc==430
    assert "`e(cmd)'"=="regress"
    capture confirm variable m, exact
    assert _rc==111
    quietly replace z=b
}

* The same weak coordinate can be legitimately omitted when the true response
* has no effect in its direction. Exact and rounded multiples are also safe.
quietly replace y=(1+(r==1)-(r==0))*2^(a+mod(g,2)+h)
quietly replace yl=3*a+mod(g,2)+h+(r==1)-(r==0)
foreach kind in weak exact_fe exact_finite {
    if "`kind'"=="weak" quietly replace z=a+2^(-41)*b
    if "`kind'"=="exact_fe" quietly replace z=mod(g,2)
    if "`kind'"=="exact_finite" quietly replace z=3*a
    quietly fereg yl a z [pw=w], absorb(g h) residuals(res)
    assert abs(res-((r==1)-(r==0)))<1e-8
    drop res
    quietly feppml y a z [pw=w], absorb(g h) separation(none) mu(m)
    assert abs(m/2^(a+mod(g,2)+h)-1)<1e-8
    drop m
}
* The identity z = g-level function + a tiny multiple of another slope
* remains exact. Reversing the explicit terms must preserve the original
* pivot convention even though the centered z would be exactly a.
quietly replace z=mod(g,2)+2^(-26)*a
foreach rhs in "a z" "z a" {
    foreach cmd in fereg feppml {
        if "`cmd'"=="fereg" quietly fereg yl `rhs' [pw=w], absorb(g h) residuals(res)
        else quietly feppml y `rhs' [pw=w], absorb(g h) separation(none) mu(res)
        assert e(N)==128 & _b[z]==0 & _se[z]==0
        assert abs(_b[a]-cond("`cmd'"=="fereg",3,ln(2)))<1e-8
        matrix B=e(b)
        _ms_omit_info B
        matrix O=r(omit)
        local iz=cond("`rhs'"=="a z",2,1)
        assert O[1,`iz']==1 & O[1,3-`iz']==0
        drop res
    }
}
* An ordinary retained indicator can also hide a continuous difference.
clear
set obs 12
generate byte cell=1+(_n>6)
generate byte rep=mod(_n-1,6)+1
generate byte y=1+mod(3*_n,7)+cell
generate byte w=1+mod(rep,3)
generate byte exposure=1+mod(_n,4)
generate double weak=cell+2^(-45)*rep
quietly feppml y ib1.cell c.rep [pw=w], vce(cluster cell) exposure(exposure) separation(none)
assert abs(_se[2.cell]-.009957359881692)<1e-9
foreach rhs in "ib1.cell c.weak" "c.weak ib1.cell" {
    capture noisily feppml y `rhs' [pw=w], vce(cluster cell) exposure(exposure) separation(none)
    assert _rc==430
    assert abs(_se[2.cell]-.009957359881692)<1e-9
}

* Five levels require the full ordinary factor span to remove the group location.
clear
set obs 30
generate byte cell=ceil(_n/6)
generate byte rep=1+mod(_n-1,6)
generate double y=1+2*cell+rep+mod(_n,2)
generate double yl=.6*cell+.1*rep+mod(_n,2)
generate byte w=1+mod(rep,3)
generate double weak=cell+2^(-44)*rep
foreach fit in fereg feppml {
    if "`fit'"=="fereg" quietly fereg yl i.cell c.rep [pw=w], residuals(control)
    else quietly feppml y i.cell c.rep [pw=w], separation(none) mu(control)
    * Independent slopes: within-cell WLS and the one-dimensional profiled
    * Poisson score, which matches the observed y-weighted mean of rep.
    if "`fit'"=="fereg" assert abs(_b[rep]-.1)<1e-12
    else assert abs(_b[rep]-.09269051007694325175)<1e-11
    foreach rhs in "i.cell c.weak" "c.weak i.cell" {
        if "`fit'"=="fereg" capture noisily fereg yl `rhs' [pw=w], residuals(candidate)
        else capture noisily feppml y `rhs' [pw=w], separation(none) mu(candidate)
        assert _rc==430
        capture confirm variable candidate, exact
        assert _rc==111
    }
    drop control
}
* With only level 2 specified, arbitrary within-cell centering is outside the
* fitted model. A second exact copy supplies a genuine numerical omission.
generate double duplicate=2*weak
foreach fit in fereg feppml {
    if "`fit'"=="fereg" {
        quietly fereg yl 2.cell c.weak [pw=w], residuals(control)
        quietly fereg yl 2.cell c.weak c.duplicate [pw=w], residuals(candidate)
    }
    else {
        quietly feppml y 2.cell c.weak [pw=w], separation(none) mu(control)
        quietly feppml y 2.cell c.weak c.duplicate [pw=w], separation(none) mu(candidate)
    }
    assert abs(control-candidate)<1e-10
    drop control candidate
}
display as result "END_FIXTURE|omitted_direction_score|rc=0"
