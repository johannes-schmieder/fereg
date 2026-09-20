version 19.0
clear all
set more off
program define _prediction_probe_repost, eclass
    args coefficients
    ereturn repost b=`coefficients'
end
set seed 493817
set obs 16400
generate long id = _n
generate int g = mod(_n,31)+1
generate byte c = mod(_n,4)+1
forvalues j=1/10 {
    generate double x`j' = rnormal()
}
generate double duplicate = x1
generate double y = 1.3*x1 - .7*x2 + sin(g) + rnormal()
replace y = . in 1
replace x3 = . in 2
tsset id

foreach model in numeric collinear factor {
    local regressors x1-x10
    if "`model'" == "collinear" local regressors x1-x10 duplicate
    if "`model'" == "factor" local regressors x1-x10 i.c
    quietly fereg y `regressors', absorb(g) residuals(resid) threads(16)
    foreach changed in no yes {
        preserve
        if "`changed'" == "yes" {
            replace x1 = x1 + .137 if mod(id,2)
            replace x2 = . in 7
            replace duplicate = . in 9
            gsort -id
            sort id
        }
        foreach restriction in all if in both {
            local restriction_if
            local restriction_in
            if inlist("`restriction'", "if", "both") local restriction_if if mod(id,3)
            if inlist("`restriction'", "in", "both") local restriction_in in 5/15001
            tempvar marked
            generate byte `marked' = 0
            replace `marked' = 1 `restriction_if' `restriction_in'
            foreach storage in double float long {
                quietly _predict `storage' reference if `marked' & e(sample), xb
                quietly predict `storage' candidate `restriction_if' `restriction_in', xb
                assert missing(reference) == missing(candidate)
                if "`storage'" == "double" assert reldif(reference,candidate) <= 1e-12 if !missing(reference)
                else assert reference == candidate
                drop reference candidate
            }
            quietly _predict double reference_xb if `marked' & e(sample), xb
            quietly predict double combined `restriction_if' `restriction_in', xbd
            quietly predict double absorbed `restriction_if' `restriction_in', d
            quietly predict double residual `restriction_if' `restriction_in', residuals
            assert combined == y-resid if `marked' & e(sample)
            assert missing(absorbed) == missing(y-reference_xb-resid) if `marked' & e(sample)
            assert reldif(absorbed, y-reference_xb-resid) <= 1e-12 if `marked' & e(sample) & !missing(absorbed)
            assert residual == resid if `marked' & e(sample)
            assert missing(combined) & missing(absorbed) & missing(residual) if !`marked' | !e(sample)
            drop reference_xb combined absorbed residual `marked'
        }
        restore
    }
    if "`model'" == "numeric" {
        preserve
        matrix original_b = e(b)
        matrix zero_b = e(b)
        matrix zero_b[1,1] = 0
        _prediction_probe_repost zero_b
        replace x1 = . in 7
        quietly _predict double reference if e(sample), xb
        quietly predict double candidate, xb
        assert missing(reference) == missing(candidate)
        assert reldif(reference,candidate) <= 1e-12 if !missing(reference)
        _prediction_probe_repost original_b
        restore
    }
    drop resid
}
display as result "END_FIXTURE|ols_prediction_scoring|rc=0"
