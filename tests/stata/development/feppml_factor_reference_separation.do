version 18
* Synthetic ordinary factor, two FEs and a zero-only singleton reference level.
* The nonbase slope and its robust SE are invariant to the level chosen as base.
set more off
clear
set seed 2639057
set obs 2001
generate int g = runiformint(1,19)
generate int h = runiformint(1,21)
generate byte ph = cond(_n==1,1,runiformint(2,44))
generate double x = rnormal()
generate int y = rpoisson(exp(0.1*x))
replace y=0 in 1
generate double w = 0.7+runiform()
forvalues j=1/12 {
    generate byte a`j' = runiformint(0,4)
}
quietly feppml y x i.(a1-a12) ib2.ph [pw=w], absorb(g h) vce(robust)
assert e(N)==2000
generate byte es1=e(sample)
assert es1==(_n!=1)
scalar b_nonbase=_b[x]
scalar se_nonbase=_se[x]
quietly feppml y x i.(a1-a12 ph) [pw=w], absorb(g h) vce(robust)
assert e(N)==2000
assert e(sample)==es1
assert reldif(_b[x],b_nonbase)<1e-8
assert reldif(_se[x],se_nonbase)<1e-8
quietly feppml y x i.(a1-a12 ph) [pw=w], absorb(g h) vce(robust) separation(fe ir)
assert e(sample)==es1
assert reldif(_b[x],b_nonbase)<1e-8
assert reldif(_se[x],se_nonbase)<1e-8
* With only level 2 present in the model, all other levels share an intercept.
quietly feppml y x i.(a1-a12) 2.ph [pw=w], absorb(g h) vce(robust)
assert e(N)==2001
assert e(sample)==1
* The second level-1 observation prevents separation of the original zero row.
replace ph=1 in 2
replace y=1 in 2
quietly feppml y x i.(a1-a12 ph) [pw=w], absorb(g h) vce(robust)
assert e(N)==2001
assert e(sample)==1
display as result "END_FIXTURE|feppml_factor_reference_separation|rc=0"
