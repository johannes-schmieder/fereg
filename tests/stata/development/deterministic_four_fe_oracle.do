version 18
clear all
set more off
set obs 4096
generate int g1 = 1 + mod(_n-1,13)
generate int g2 = 1 + mod(floor((_n-1)/13),11)
generate int g3 = 1 + mod(7*(_n-1)+floor(_n/17),7)
generate int g4 = 1 + mod(3*(_n-1)+floor(_n/23),5)
generate double x1 = sin(_n/17)+mod(_n,7)/5
generate double x2 = cos(_n/29)-mod(_n,11)/9
generate double y = .7*x1-.4*x2+.03*g1-.05*g2+.07*g3+.11*g4+sin(_n/7)
generate double count = 1+mod(floor(100*exp(.2*x1-.1*x2+.03*g1+.02*g2)),19)
generate int fw = 1+mod(_n,5)
generate double raw_w = 1
generate double oracle_r = .

mata:
void four_fe_oracle(string scalar model, string scalar kind)
{
    X=st_data(.,("x1","x2"))
    for (q=1;q<=4;q++) {
        g=st_data(.,"g"+strofreal(q)); L=max(g)
        // Include all first-factor levels and drop one level for subsequent factors.
        for (c=(q==1 ? 1 : 2);c<=L;c++) X=(X,g:==c)
    }
    w=st_data(.,"raw_w")
    if (model=="ols") {
        y=st_data(.,"y"); A=invsym(quadcross(X,w,X))
        b=A*quadcross(X,w,y); r=y-X*b
        st_matrix("oracle_b",b[1..2]'); st_store(.,"oracle_r",r)
    }
    else {
        y=st_data(.,"count"); mu=st_data(.,"mu_dummy")
        A=invsym(quadcross(X,w:*mu,X)); r=y-mu
    }
    sw=w:^2
    if (kind=="fw") sw=w
    V=A*quadcross(X,sw:*r:^2,X)*A*st_numscalar("correction")
    st_matrix("oracle_V",V[1..2,1..2])
}
end

foreach kind in unit fw {
    local weight ""
    replace raw_w = 1
    if "`kind'"=="fw" {
        local weight "[fw=fw]"
        replace raw_w = fw
    }
    quietly poisson count x1 x2 ibn.g1 ib1.g2 ib1.g3 ib1.g4 `weight', noconstant nolog
    matrix poisson_b=e(b)
    predict double mu_dummy, n
    foreach mode in auto fast low {
        foreach t in 1 4 8 16 {
            forvalues repeat=1/2 {
                quietly fereg y x1 x2 `weight', absorb(g1 g2 g3 g4) ///
                    solver(lsmr) deterministic threads(`t') memory(`mode') ///
                    vce(robust) residuals(r)
                scalar correction=e(N)/e(df_r)
                mata: four_fe_oracle("ols","`kind'")
                matrix finite_b=e(b)[1,1..2]
                matrix finite_V=e(V)[1..2,1..2]
                assert mreldif(finite_b,oracle_b)<1e-7
                assert mreldif(finite_V,oracle_V)<1e-6
                assert abs(r-oracle_r)<1e-7*(1+abs(oracle_r))
                assert e(sample)==1
                if `repeat'==1 {
                    matrix held_b=e(b)
                    matrix held_v=e(V)
                    generate double held_r=r
                }
                else {
                    assert mreldif(e(b),held_b)==0
                    assert mreldif(e(V),held_v)==0
                    assert r==held_r
                    drop held_r
                }
                drop r
                quietly feppml count x1 x2 `weight', absorb(g1 g2 g3 g4) ///
                    deterministic threads(`t') memory(`mode') separation(none) ///
                    vce(robust) mu(mu)
                assert e(deterministic)==1
                scalar correction=e(feppml_inference)[3,1]
                mata: four_fe_oracle("ppml","`kind'")
                forvalues j=1/2 {
                    assert abs(e(b)[1,`j']-poisson_b[1,`j'])<1e-7
                }
                matrix finite_V=e(V)[1..2,1..2]
                assert mreldif(finite_V,oracle_V)<1e-6
                assert abs(mu-mu_dummy)<1e-7*(1+abs(mu_dummy))
                assert e(sample)==1
                if `repeat'==1 {
                    matrix held_pb=e(b)
                    matrix held_pv=e(V)
                    generate double held_mu=mu
                }
                else {
                    assert mreldif(e(b),held_pb)==0
                    assert mreldif(e(V),held_pv)==0
                    assert mu==held_mu
                    drop held_mu
                }
                drop mu
            }
        }
    }
    drop mu_dummy
}
display as result "END_FIXTURE|deterministic_four_fe_oracle|rc=0"
