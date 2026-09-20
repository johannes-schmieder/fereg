version 18.0
clear all
set more off
set seed 290620
set obs 16416
generate long row = _n
generate int g = mod(_n,32)
generate int h = mod(floor((_n-1)/32),23)
generate byte q = mod(_n,11)
generate byte cl = mod(_n,37)
generate byte cl2 = mod(floor((_n-1)/7),43)
generate double w = 1 + mod(_n,5)
forvalues j=1/10 {
    generate double x`j' = rnormal()
}
generate double y = x1/3 - x2/5 + g/20 - h/13 + q/9 + rnormal()

// Cross the row-pipeline threshold with two and three effects. The restricted
// command uses the ordinary loader; explicit dummies provide a separate oracle.
foreach effects in "g h" "g h q" {
    local dummies
    foreach effect of local effects {
        local dummies `dummies' i.`effect'
    }
    foreach vce in "" "robust" "cluster cl" {
        local vcopt
        if "`vce'" != "" local vcopt vce(`vce')
        quietly regress y x1-x10 `dummies', `vcopt'
        matrix oracle_b = e(b)
        matrix oracle_v = e(V)
        scalar oracle_rss = e(rss)
        scalar oracle_df = e(df_r)
        foreach t in 4 16 {
            quietly fereg y x1-x10, absorb(`effects') threads(`t') `vcopt'
            assert e(N) == _N
            assert e(df_r) == oracle_df
            assert abs(e(rss)-oracle_rss) < 1e-9*oracle_rss
            forvalues j=1/10 {
                assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-9
                forvalues k=1/10 {
                    assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
                }
            }
            matrix piped_b = e(b)
            matrix piped_v = e(V)
            quietly fereg y x1-x10 if 1, absorb(`effects') threads(`t') `vcopt'
            assert mreldif(e(b),piped_b) == 0
            assert mreldif(e(V),piped_v) == 0
            assert row == _n
        }
    }
}

// Both saved channels and crossing clusters must survive the alternate loader
// without changing normalization, fitted values, or excluded-row publication.
quietly fereg y x1-x10, absorb(a=g b=h) threads(16) vce(cluster cl cl2) residuals(r)
matrix saved_b = e(b)
matrix saved_v = e(V)
quietly fereg y x1-x10 if 1, absorb(c=g d=h) threads(16) vce(cluster cl cl2) residuals(s)
assert mreldif(e(b),saved_b) == 0
assert mreldif(e(V),saved_v) == 0
assert a == c & b == d & r == s
drop a b c d r s

// A string FE excludes row transport even though the model has multiple
// ordinary intercept effects and enough numeric regressors for RHS reuse.
quietly fereg y x1-x10, absorb(g h) threads(16) vce(robust)
matrix numeric_b = e(b)
matrix numeric_v = e(V)
generate str8 h_label = string(h)
quietly fereg y x1-x10, absorb(g h_label) threads(16) vce(robust)
assert mreldif(e(b),numeric_b) < 1e-10
assert mreldif(e(V),numeric_v) < 1e-10
assert e(N) == _N
drop h_label

// Missingness and cascading singletons invalidate the speculative original-axis
// matrix. The two-row component disappears through two successive removals.
quietly replace g = 999 in 1
quietly replace h = 999 in 1/2
quietly replace x10 = .z in 3
quietly regress y x1-x10 i.g i.h if row > 2, vce(robust)
matrix oracle_b = e(b)
matrix oracle_v = e(V)
generate byte oracle_sample = e(sample)
quietly fereg y x1-x10, absorb(a=g b=h) threads(16) vce(robust) residuals(r)
assert e(sample) == oracle_sample
assert missing(a) & missing(b) & missing(r) if !oracle_sample
forvalues j=1/10 {
    assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-9
    forvalues k=1/10 {
        assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
    }
}
drop a b r oracle_sample

// Supplied weights, explicit restrictions, and low-memory execution keep their
// existing paths and agree with an independent retained-sample dummy regression.
foreach route in weighted restricted low {
    local wt
    local restriction if row > 3
    local options
    if "`route'" == "weighted" local wt [aw=w]
    if "`route'" == "restricted" local restriction if row > 100
    local oracle_restriction `restriction'
    if "`route'" == "low" {
        local restriction
        local oracle_restriction if row > 2
        local options memory(low)
    }
    quietly regress y x1-x10 i.g i.h `oracle_restriction' `wt', vce(robust)
    matrix oracle_b = e(b)
    matrix oracle_v = e(V)
    generate byte oracle_sample = e(sample)
    quietly fereg y x1-x10 `restriction' `wt', absorb(g h) threads(16) vce(robust) `options'
    assert e(sample) == oracle_sample
    forvalues j=1/10 {
        assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-9
        forvalues k=1/10 {
            assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
        }
    }
    drop oracle_sample
}
assert row == _n
display as result "END_FIXTURE|ols_multi_fe_input|rc=0"
