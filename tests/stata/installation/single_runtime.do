version 18.0
clear all
set more off
set obs 96
generate double x = mod(7 * _n, 29) / 14 - 1
generate double y = 0.7 + mod(11 * _n, 23) / 5
generate byte g = mod(_n, 8)
quietly regress y c.x i.g
scalar oracle_x = _b[x]
scalar oracle_vx = e(V)[1,1]
scalar oracle_df = e(df_r)
quietly fereg y c.x, absorb(g) threads(1) deterministic
matrix ols_b = e(b)
matrix ols_v = e(V)
assert abs(ols_b[1,1]-oracle_x)<1e-12
assert abs(ols_v[1,1]-oracle_vx)<1e-12
assert e(df_r)==oracle_df
generate byte ols_sample = e(sample)
estimates store saved_ols
quietly feppml y c.x, absorb(g) threads(1) deterministic
matrix ppml_b = e(b)
matrix ppml_v = e(V)
generate byte ppml_sample = e(sample)
forvalues repeat = 1/3 {
    quietly fereg y c.x, absorb(g) threads(1) deterministic
    assert mreldif(e(b), ols_b)==0 & mreldif(e(V), ols_v)==0
    assert e(sample)==ols_sample
    quietly feppml y c.x, absorb(g) threads(1) deterministic
    assert mreldif(e(b), ppml_b)==0 & mreldif(e(V), ppml_v)==0
    assert e(sample)==ppml_sample
    quietly fereg, selftest threads(1)
    assert strpos("`r(plugin_name)'", "fereg_")==1
    assert strpos("`r(plugin_name)'", "legacy")==0
    assert mreldif(e(b), ppml_b)==0 & mreldif(e(V), ppml_v)==0
    assert e(sample)==ppml_sample
    foreach retired in version doctor {
        capture noisily fereg, `retired'
        assert _rc==198
        assert mreldif(e(b), ppml_b)==0 & mreldif(e(V), ppml_v)==0
        assert e(sample)==ppml_sample
    }
}
* Retired options fail before publication and preserve the prior transaction.
foreach command in fereg feppml {
    foreach removed in next legacy malformed {
        capture noisily `command' y c.x, engine(`removed')
        assert _rc==198
        assert mreldif(e(b), ppml_b)==0 & mreldif(e(V), ppml_v)==0
        assert e(sample)==ppml_sample
        capture noisily fereg, selftest engine(`removed')
        assert _rc==198
    }
}
estimates restore saved_ols
assert mreldif(e(b), ols_b)==0 & mreldif(e(V), ols_v)==0
assert e(sample)==ols_sample
di as result "END_FIXTURE|single_runtime|rc=0"
