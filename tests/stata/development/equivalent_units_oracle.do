version 18
clear all
set more off

* Each cell has outcomes 0.5 and 1.5 times its true mean with equal weights.
* Its score vanishes at this known fit. Clusters divide the cell, so there
* is sampling variation for the slope. Build the sandwich from the known
* means, explicit FE indicators, and observation-level cluster assignments.
set obs 504
generate int replicate = mod(_n-1,2)
generate int g = mod(floor((_n-1)/2),7)
generate int h = mod(floor((_n-1)/14),6)
generate int x0 = mod(floor((_n-1)/84),3)
generate long cl = 1+mod(3*replicate+g+2*h+x0,11)
generate double truth = 2^(1+mod(g,2)+mod(h,2)+x0)
generate double y = truth*(.5+replicate)
generate double w0 = 1+mod(g+2*h+x0,3)
generate double off = 0
generate double x = x0
generate double w = w0
generate double linear_truth = .375*x0 + g/2 - h/4
generate double linear_y = linear_truth + (replicate-.5)*(1+mod(x0+h,2))
quietly regress linear_y x0 i.g i.h [pw=w0], vce(cluster cl)
scalar linear_se_truth = _se[x0]
mata:
    G = st_data(., "g")
    H = st_data(., "h")
    X = st_data(., "x0")
    for (i=0; i<7; ++i) X = X, (G:==i)
    for (i=1; i<6; ++i) X = X, (H:==i)
    w = st_data(., "w0")
    mu = st_data(., "truth")
    y = st_data(., "y")
    cl = st_data(., "cl")
    B = invsym(quadcross(X, w:*mu, X))
    U = J(11, cols(X), 0)
    for (i=1; i<=11; ++i) U[i,.] = quadcross(w:*(y-mu):*(cl:==i), X)
    V = (11/10)*B*quadcross(U,U)*B
    st_numscalar("se_truth", sqrt(V[1,1]))
end

foreach cell in base xbig xsmall xshift wbig wsmall offpos offneg offfe combo {
    local scale = 1
    local loc = 0
    if "`cell'"=="xbig" local scale = 2^24
    if inlist("`cell'","xsmall","combo") local scale = 2^(-20)
    if "`cell'"=="xshift" local loc = 2^24
    quietly replace x = x0*`scale'+`loc'
    quietly replace w = w0
    if "`cell'"=="wbig" quietly replace w = w0*2^32
    if inlist("`cell'","wsmall","combo") quietly replace w = w0*2^(-32)
    quietly replace off = 0
    if "`cell'"=="offpos" quietly replace off = 500
    if "`cell'"=="offneg" quietly replace off = -500
    if inlist("`cell'","offfe","combo") quietly replace off = (g-3)*100+(h-2)*31
    quietly feppml y x [pw=w], absorb(g h) offset(off) vce(cluster cl) mu(pred) threads(2)
    assert e(N)==504 & e(sample)==1
    assert abs(_b[x]*`scale'-ln(2)) < 1e-10
    assert reldif(_se[x]*`scale', se_truth) < 1e-10
    assert abs(pred/truth-1) < 1e-8
    drop pred
    generate double adjusted_y = linear_y-off
    quietly fereg adjusted_y x [pw=w], absorb(g h) vce(cluster cl) residuals(resid) threads(2)
    assert e(N)==504 & e(sample)==1
    assert abs(_b[x]*`scale'-.375) < 1e-10
    assert reldif(_se[x]*`scale', linear_se_truth) < 1e-10
    assert abs(resid - (linear_y-linear_truth)) < 1e-8
    drop resid adjusted_y
}
display as result "END_FIXTURE|equivalent_units_oracle|rc=0"
