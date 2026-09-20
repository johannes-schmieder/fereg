version 19.0
clear all
set more off
set seed 731913
set obs 16400
generate long id = _n
generate int g = mod(_n,31)+1
generate int h = mod(floor((_n-1)/31),7)+1
generate int c = mod(_n,29)+1
generate str8 gs = string(g)
generate double w = mod(_n,5)+1
forvalues j=1/10 {
    generate double x`j' = rnormal() + sin(g/`j')
}
generate double y = 1.7*x1 - .8*x2 + sin(g) + .3*cos(h) + rnormal()
foreach sample in complete missing {
    preserve
    if "`sample'" == "missing" {
        replace y = . in 1
        replace x3 = . in 2
        replace g = . in 3
        replace gs = "" in 3
        replace c = . in 4
    }
    egen long gh = group(g h)

foreach effect in numeric string interaction {
    local absorb g
    local dummy g
    if "`effect'" == "string" local absorb gs
    if "`effect'" == "interaction" {
        local absorb g#h
        local dummy gh
    }
    foreach weight in unit aw fw pw {
        local wspec
        local meanweight
        if "`weight'" != "unit" local wspec [`weight'=w]
        if "`weight'" != "unit" local meanweight [aw=w]
        foreach covariance in iid robust cluster {
            if "`weight'" == "pw" & "`covariance'" == "iid" continue
            local vce
            if "`covariance'" == "robust" local vce vce(robust)
            if "`covariance'" == "cluster" local vce vce(cluster c)
            quietly regress y x1-x10 i.`dummy' `wspec', `vce'
            matrix oracle_b = e(b)
            matrix oracle_V = e(V)
            quietly predict double oracle_resid if e(sample), residuals
            generate byte oracle_sample = e(sample)
            quietly fereg y x1-x10 `wspec', absorb(saved=`absorb') ///
                residuals(resid) `vce' threads(16)
            assert e(sample) == oracle_sample
            forvalues j=1/10 {
                assert reldif(_b[x`j'], oracle_b[1,`j']) <= 1e-9
                forvalues k=1/10 {
                    assert abs(e(V)[`j',`k']-oracle_V[`j',`k']) <= 1e-9
                }
            }
            assert abs(resid-oracle_resid) <= 1e-9 if e(sample)
            quietly predict double finite, xb
            quietly predict double combined, xbd
            assert abs(y-finite-saved-resid) <= 1e-9 if e(sample)
            assert abs(combined-finite-saved) <= 1e-9 if e(sample)
            quietly summarize saved if e(sample) `meanweight', meanonly
            assert abs(r(mean)) <= 1e-9
            drop oracle_resid oracle_sample saved resid finite combined
        }
    }
}
    restore
}
display as result "END_FIXTURE|ols_one_fe_final_recovery|rc=0"
