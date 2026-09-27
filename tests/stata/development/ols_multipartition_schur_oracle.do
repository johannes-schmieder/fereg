version 18.0
clear all
set more off
set obs 960
generate int g = 1 + mod(_n-1,8)
generate int h = 1 + mod(floor((_n-1)/9),6)
generate int j = 1 + mod(floor((_n-1)/13),5)
generate double z = sin(_n/13)+mod(_n,5)/7
generate double w = cos(_n/23)-mod(_n,3)/5
generate double z2 = sin(_n/11)
generate double x1 = sin(_n/17)+mod(_n,7)/5
generate double x2 = cos(_n/29)-mod(_n,11)/9
generate double y = .7*x1-.4*x2+.03*g+.07*g*z-.05*h*w+sin(_n/7)
generate double aw = .2+mod(_n,13)/3
generate double pw = aw
generate int fw = 1+mod(_n,7)
generate double raw_w = 1

mata:
void ols_slope_oracle(string scalar kind, real scalar parts)
{
    g=st_data(.,"g"); h=st_data(.,"h"); j=st_data(.,"j")
    G=J(rows(g),8,0); H=J(rows(g),6,0); JFE=J(rows(g),5,0)
    for (c=1;c<=8;c++) G[.,c]=(g:==c)
    for (c=1;c<=6;c++) H[.,c]=(h:==c)
    for (c=1;c<=5;c++) JFE[.,c]=(j:==c)
    X=(st_data(.,("x1","x2")),G,H[.,2..6],G:*st_data(.,"z"),G:*st_data(.,"z2"),H:*st_data(.,"w"))
    if (parts==3) X=(X,JFE[.,2..5])
    y=st_data(.,"y"); w=st_data(.,"raw_w")
    A=invsym(quadcross(X,w,X))
    b=A*quadcross(X,w,y); r=y-X*b
    sw=w:^2
    if (kind=="fw") sw=w
    V=A*quadcross(X,sw:*r:^2,X)*A*st_numscalar("correction")
    st_matrix("oracle_b",b[1..2]')
    st_matrix("oracle_V",V[1..2,1..2])
    st_store(.,"oracle_r",r)
}
end
generate double oracle_r = .
foreach parts in 2 3 {
foreach kind in unit aw fw pw {
    local weight ""
    replace raw_w = 1
    if "`kind'" != "unit" {
        local weight "[`kind'=`kind']"
        replace raw_w = `kind'
    }
    foreach mode in fast low {
    foreach t in 1 4 {
        local third ""
        if `parts'==3 local third "c=j"
        fereg y x1 x2 `weight', absorb(a=g##c.(z z2) b=h##c.w `third') ///
            threads(`t') memory(`mode') poolsize(1) vce(robust) residuals(r)
        assert e(converged)==1
        assert e(sample)==1
        assert e(fereg_certificate)[32,2]==1
        scalar correction = e(N)/e(df_r)
        mata: ols_slope_oracle("`kind'", `parts')
        matrix finite_b=e(b)[1,1..2]
        matrix finite_V=e(V)[1..2,1..2]
        assert mreldif(finite_b,oracle_b)<1e-9
        assert mreldif(finite_V,oracle_V)<1e-9
        assert abs(r-oracle_r)<1e-8
        predict double fitted, xbd
        assert abs(y-fitted-r)<1e-8
        drop a a_Slope1 a_Slope2 b b_Slope1 r fitted
        if `parts'==3 drop c
    }
    }
}
}
display as result "END_FIXTURE|ols_multipartition_schur_oracle|rc=0"
