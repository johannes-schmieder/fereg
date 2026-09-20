version 19.0
clear all
set more off
set seed 83711
set obs 16400
generate long id = _n
generate int g = mod(_n,71)+1
generate double w = mod(_n,5)+1
forvalues j=1/10 {
    generate double x`j' = rnormal()
}
generate double y = 1.2*x1 - .3*x2 + sin(g) + rnormal()

foreach sample in complete filtered {
    preserve
    local restriction
    if "`sample'" == "filtered" {
        replace y = . in 1
        replace x3 = . in 2
        replace g = . in 3
        replace w = 0 in 4
        replace w = . in 5
        replace g = 1000 in 7
        local restriction if mod(id,3) != 0 in 1/16397
    }
    foreach weight in unit aweight fweight pweight {
        if "`sample'" == "filtered" replace w = cond("`weight'" == "aweight", 0, 1) in 4
        local weight_option
        if "`weight'" != "unit" local weight_option `weight'(w)
        quietly _fereg_estimate_ols_test y x1-x10 `restriction', ///
            absorb(saved=g) residual(resid) xb(finite) xbd(combined) ///
            d(absorbed) vce(robust) `weight_option' threads(16)
        assert missing(saved) & missing(resid) & missing(finite) & ///
            missing(combined) & missing(absorbed) if !e(sample)
        assert !missing(saved,resid,finite,combined,absorbed) if e(sample)
        assert abs(y-finite-saved-resid) < 1e-9 if e(sample)
        assert abs(combined-finite-absorbed) < 1e-9 if e(sample)
        assert abs(absorbed-saved) < 1e-9 if e(sample)
        drop saved resid finite combined absorbed
    }
    restore
}

quietly regress y x1
generate byte prior_sample = e(sample)
matrix prior_b = e(b)
unab prior_variables : _all
foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics precommit {
    capture quietly _fereg_estimate_ols_test y x1-x10, ///
        absorb(saved=g) residual(resid) xb(finite) xbd(combined) ///
        d(absorbed) vce(robust) threads(16) inject(`stage')
    assert _rc == 3498
    unab after_variables : _all
    assert `"`prior_variables'"' == `"`after_variables'"'
    assert `"`e(cmd)'"' == "regress"
    assert mreldif(e(b),prior_b) == 0
    assert e(sample) == prior_sample
}
display as result "END_FIXTURE|ols_private_output_initialization|rc=0"
