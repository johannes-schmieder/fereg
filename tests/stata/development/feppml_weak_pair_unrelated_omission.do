version 18
clear all
set more off

* The two t=1 cells are separated by -t. The other five cells have a
* finite optimum, so t is identically zero in the final design.
input byte (x v t y)
0  0  0  1
1  0  0  2
2  1  0  1
0  1  0  0
3 -1  0  0
0  0  1  0
1  2  1  0
end
expand 12
bysort x v t y: generate byte copy = _n
generate byte g = mod(copy,3)
generate byte h = mod(floor((copy-1)/3),2)
generate byte kept = t==0
generate double z = x + 2^-18*v
generate byte twox = 2*x

* The high-precision Poisson score solution with explicit FE indicators
* and its independently formed HC1 sandwich use the five retained cells.
quietly feppml y x v t, absorb(g h) vce(robust) threads(1)
assert e(sample)==kept
assert abs(_b[x] + .16164957769268255)<1e-10
assert abs(_b[v] + .02942573599140180)<1e-10
assert _b[t]==0 & _se[t]==0
matrix V = e(V)
assert abs(V[1,1] - .0049748717376000594)<1e-11
assert abs(V[1,2] + .0027832759106146614)<1e-11
assert abs(V[2,2] - .019632466995241172)<1e-11
quietly lincom x+v
assert abs(r(estimate) + .19107531368408435)<1e-10
assert abs(r(se) - .13798835788432265)<1e-10
quietly test x v
assert r(df)==2 & abs(r(chi2) - 6.047367274719175)<1e-8

* Seed an accepted exact alias as the transaction's previous estimate.
quietly feppml y x twox t if kept, absorb(g h) separation(none) vce(robust) threads(1)
assert e(sample)==kept
assert _se[x]>0 & _se[twox]==0 & _se[t]==0
matrix previous_b = e(b)
matrix previous_V = e(V)

* In raw coordinates the three covariance entries of x and z have a grid
* spacing of 2^-22. Even the closest representable variance for x+z misses
* its true SE (.07053277066442279) by more than 2.10e-7. Omitting t cannot
* authorize this covariance when both members of the recovered pair remain.
foreach mode in fast low {
    foreach detector in default none {
        local sample
        if "`detector'"=="none" local sample if kept
        capture noisily feppml y x z t `sample', absorb(g h) separation(`detector') ///
            vce(robust) memory(`mode') threads(1) mu(rejected_mean)
        local rc = _rc
        if `rc'==0 {
            quietly lincom x+z
            display as error "uncertified x+z SE = " %21.16g r(se)
        }
        assert `rc'==430
        mata: assert(st_matrix("e(b)") == st_matrix("previous_b"))
        mata: assert(st_matrix("e(V)") == st_matrix("previous_V"))
        assert e(sample)==kept
        confirm new variable rejected_mean
    }
}
display as result "END_FIXTURE|feppml_weak_pair_unrelated_omission|rc=0"
