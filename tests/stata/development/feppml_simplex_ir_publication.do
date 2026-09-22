version 18.0
clear all
set more off

* The remaining zero boundary exceeds the default simplex memory cap.
* Positive rows span the complete retained design, independently ruling out
* further separation; only the deliberately zero-only FE level can drop.
set obs 8192
generate int g = 1 + mod(floor((_n-1)/8),32)
generate double x1 = sin(_n/17) + cos(_n/29)
generate double x2 = cos(_n/31) + sin(_n/23)
generate double y = cond(mod(_n,4)==0, 2+mod(floor((_n-1)/4),5), 0)
replace y = 0 if g==32
generate double off = .1*sin(_n/37)
generate byte fw = 1+mod(_n,3)
generate double pw = .5+mod(_n,7)/3
generate double raw_w = 1

mata:
void simplex_ir_oracle()
{
    g = st_data(.,"g")
    keep = selectindex(g:<32)
    X = st_data(keep,tokens("x1 x2"))
    for (j=1;j<=31;j++) X = (X,g[keep]:==j)
    y = st_data(keep,"y")
    mu = st_data(keep,"mu_dummy")
    w = st_data(keep,"raw_w")
    interior = select(X,y:>0)
    assert(rank(quadcross(interior,interior))==cols(X))
    Hinv = invsym(quadcross(X,w:*mu,X))
    score_w = w:^2
    if (st_local("kind")=="fw") score_w=w
    meat = quadcross(X,score_w:*(y-mu):^2,X)
    V = Hinv*meat*Hinv*st_numscalar("correction")
    st_matrix("V_dummy",V[1..2,1..2])
}
end

foreach kind in unit fw pw {
    local weight ""
    replace raw_w=1
    if "`kind'"!="unit" {
        local weight "[`kind'=`kind']"
        replace raw_w=`kind'
    }
    foreach threads in 1 4 {
        feppml y x1 x2 `weight', absorb(g) offset(off) threads(`threads') ///
            mu(mu_native) dropreason(reason)
        assert e(converged)==1 & e(feppml_certificate)[16,1]==1
        assert e(sample)==(g<32)
        assert e(separated)==256 & e(N_physical)==7936
        assert reason==cond(g==32,9,0)
        assert missing(mu_native) if g==32
        assert e(feppml_separation_sequence)[3,7]==16
        assert e(feppml_separation_sequence)[3,13]==0
        assert e(feppml_separation_sequence)[4,7]==2
        assert e(feppml_separation_sequence)[4,13]==1
        assert e(feppml_separation_sequence)[4,12]==e(feppml_receipt)[1,24]
        matrix b_native=e(b)
        matrix V_native=e(V)
        scalar correction=e(feppml_inference)[3,1]
        assert abs(correction-e(N_effective)/(e(N_effective)-1))<1e-12
        quietly poisson y x1 x2 ibn.g if g<32 `weight', noconstant offset(off) ///
            nolog tolerance(1e-12) ltolerance(1e-12) nrtolerance(1e-12)
        forvalues j=1/2 {
            assert abs(_b[x`j']-b_native[1,`j'])<=1e-10+1e-7*abs(_b[x`j'])
        }
        predict double mu_dummy,n
        assert abs(mu_native-mu_dummy)<=1e-7*(1+abs(mu_dummy)) if g<32
        mata: simplex_ir_oracle()
        mata: assert(all(abs(st_matrix("V_native")[1..2,1..2]-st_matrix("V_dummy")) :<= 1e-10:+1e-6:*abs(st_matrix("V_dummy"))))
        drop mu_native mu_dummy reason
    }
}

* A skipped simplex-only request still fails atomically; no IR is substituted.
quietly regress y x1 x2
matrix before_b=e(b)
matrix before_V=e(V)
generate byte before_sample=e(sample)
capture noisily feppml y x1 x2, absorb(g) separation(simplex) ///
    mu(rejected_mu) dropreason(rejected_reason)
assert _rc==909
assert "`e(cmd)'"=="regress"
assert mreldif(e(b),before_b)==0 & mreldif(e(V),before_V)==0
assert e(sample)==before_sample
confirm new variable rejected_mu
confirm new variable rejected_reason
display as result "END_FIXTURE|feppml_simplex_ir_publication|rc=0"
