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
    quietly feppml y a z [pw=w], absorb(g h) memory(`memory') separation(none) mu(m)
    assert abs(m/mu-1)<1e-8
    drop m
    foreach kind in fe finite {
        if "`kind'"=="fe" quietly replace z=mod(g,2)+2^(-26)*b
        else quietly replace z=a+2^(-41)*b
        * Both recodings preserve the true direction b exactly in binary64.
        * The existing numerical rank choice omits it in these coordinates.
        quietly regress yl a
        capture noisily fereg yl a z [pw=w], absorb(g h) memory(`memory') residuals(res)
        assert _rc==430
        assert "`e(cmd)'"=="regress"
        capture confirm variable res, exact
        assert _rc==111
        capture noisily feppml y a z [pw=w], absorb(g h) memory(`memory') separation(none) mu(m)
        assert _rc==430
        assert "`e(cmd)'"=="regress"
        capture confirm variable m, exact
        assert _rc==111
    }
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
