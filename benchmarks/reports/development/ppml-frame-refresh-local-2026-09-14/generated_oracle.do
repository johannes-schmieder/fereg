version 18.0
clear all
set more off
set obs 4099
generate long obs_id = _n
generate byte g=mod(_n,8)
generate byte f=mod(floor((_n-1)/8),3)
generate double off=0.1*sin(_n/21)
forvalues j=1/7 {
    generate double x`j'=cos(_n/(17+`j'))+0.1*sin(_n/(31+`j'))
}
generate double y=1+mod(_n,17)
quietly poisson y x1-x7 i.f ibn.g, noconstant offset(off) vce(robust) nolog
matrix oracle_b=e(b)
matrix oracle_V=e(V)
predict double mu_oracle, n
quietly feppml y x1-x7 i.f, absorb(g) offset(off) vce(robust) separation(none) keepsingletons threads(4) mu(mu_native)
assert e(converged)==1
assert e(feppml_certificate)[16,1]==1
assert e(sample)==1
assert abs(mu_native-mu_oracle)<=1e-7*(1+abs(mu_oracle))
matrix native_b=e(b)
matrix native_V=e(V)
foreach name in x1 x2 x3 x4 x5 x6 x7 1.f 2.f {
    local a=colnumb(native_b,"`name'")
    local b=colnumb(oracle_b,"`name'")
    assert `a'<. & `b'<.
    assert abs(native_b[1,`a']-oracle_b[1,`b'])<1e-7
    foreach other in x1 x2 x3 x4 x5 x6 x7 1.f 2.f {
        local c=colnumb(native_b,"`other'")
        local d=colnumb(oracle_b,"`other'")
        assert abs(native_V[`a',`c']-oracle_V[`b',`d'])<1e-7
    }
}
display "END_FIXTURE|predictor_generated|rc=0"
