version 18.0
clear all
set more off
set obs 6144
generate long pair = floor((_n-1)/2)
generate int g = 1 + floor(pair/6)
generate int h = 1 + mod(floor(pair/6)+13*mod(pair,6),80)
forvalues j=1/4 {
    generate double x`j' = sin(pair*(.17+.09*`j')) + .3*cos(pair/(7+`j'))
}
generate double truth_eta = .1*x1-.2*x2+.07*x3+.03*x4+.03*g/512-.05*h/80
generate double truth_mu = exp(truth_eta)
generate double q = 1
generate double err = .03*sin(pair*.71)
generate double y = truth_mu
mata:
void canonical_truth_vce()
{
    g=st_data(.,"g"); h=st_data(.,"h")
    G=J(rows(g),512,0); H=J(rows(h),79,0)
    for (i=1; i<=rows(g); i++) {
        G[i,g[i]]=1
        if (h[i]>1) H[i,h[i]-1]=1
    }
    X=(st_data(.,("x1","x2","x3","x4")),G,H)
    w=st_data(.,"q"); mu=st_data(.,"truth_mu"); y=st_data(.,"y")
    bread=invsym(quadcross(X,w:*mu,X))
    meat=quadcross(X,(w:*(y-mu)):^2,X)
    V=bread*meat*bread*st_numscalar("correction")
    st_matrix("V_truth",V[1..4,1..4])
}
end
foreach kind in unit pw {
    local weight ""
    replace q=1
    if "`kind'"=="pw" {
        replace q=cond(mod(_n,2)==1,.7,2.3)*(.3+mod(pair,13))
        local weight "[pw=q]"
    }
    replace y=truth_mu+cond(mod(_n,2)==1,err/q,-err/q)
    feppml y x1 x2 x3 x4 `weight', absorb(g h) keepsingletons separation(none) ///
        solver(map) memory(low) poolsize(4) threads(4) vce(robust) mu(mu_native)
    assert e(converged)==1
    assert e(feppml_certificate)[16,1]==1
    assert e(sample)==1
    assert abs(mu_native-truth_mu)<=1e-8*(1+truth_mu)
    matrix b=e(b)
    assert abs(b[1,1]-.1)<1e-9
    assert abs(b[1,2]+.2)<1e-9
    assert abs(b[1,3]-.07)<1e-9
    assert abs(b[1,4]-.03)<1e-9
    scalar correction=e(feppml_inference)[3,1]
    matrix V_native=e(V)
    matrix V_finite=V_native[1..4,1..4]
    mata: canonical_truth_vce()
    assert mreldif(V_finite,V_truth)<1e-8
    drop mu_native
}
display as result "END_FIXTURE|feppml_iterative_canonical_oracle|rc=0"
