version 18.0
clear all
set more off
set obs 600
generate double y = cond(_n<=257, 0, 1+mod(_n,3))
forvalues j=1/257 {
    generate byte x`j' = (_n==`j')
}
quietly summarize y if _n>257
scalar oracle_b = ln(r(mean))
scalar oracle_v = r(Var)/(r(N)*r(mean)^2)
foreach workers in 1 4 {
    feppml y x*, separation(simplex) keepsingletons threads(`workers')
    assert e(N) == 343
    assert e(sample) == (_n>257)
    assert abs(_b[_cons]-oracle_b) < 1e-10
    assert abs(_se[_cons]^2-oracle_v) < 1e-10
}
display as result "END_FIXTURE|feppml_long_separation_report_bank|rc=0"
