*! fereg prediction adapter 0.5.0 24aug2026
program define fereg_p
    version 18.0
    if !inlist(`"`e(cmd)'"', "fereg", "_fereg_estimate_ols_test", ///
        "_fereg_ols_hidden_numeric_v1") {
        error 301
    }
    syntax newvarname [if] [in] [, XB XBD D RESiduals]

    local requested = (`"`xb'"' != "") + (`"`xbd'"' != "") + ///
        (`"`d'"' != "") + (`"`residuals'"' != "")
    if `requested' > 1 {
        di as error "only one prediction statistic may be specified"
        exit 198
    }
    if `requested' == 0 local xb "xb"

    local sample "e(sample)"
    if `"`if'`in'"' != "" {
        marksample touse, novarlist
        local sample "`touse' & e(sample)"
    }
    // Compute in double, store into a private variable of the requested type,
    // and publish only after every finite value proved representable.
    tempvar work stored
    if `"`xb'"' != "" {
        quietly _fereg_p_xb double `work' if `sample', storage(`typlist')
    }
    else {
        local residual_var `"`e(resid)'"'
        if strtrim(`"`residual_var'"') == "" {
            di as error "predict `xbd'`d'`residuals' requires residuals() at estimation time"
            exit 498
        }
        capture confirm numeric variable `residual_var'
        if _rc {
            di as error "the estimation residual variable is no longer available"
            exit 111
        }
        if `"`residuals'"' != "" {
            quietly generate double `work' = `residual_var' if `sample'
        }
        else if `"`xbd'"' != "" {
            quietly generate double `work' = `e(depvar)' - `residual_var' if `sample'
        }
        else {
            tempvar linear_prediction
            quietly _fereg_p_xb double `linear_prediction' if `sample', storage(double)
            quietly generate double `work' = ///
                `e(depvar)' - `linear_prediction' - `residual_var' if `sample'
        }
    }
    quietly generate `typlist' `stored' = `work' if `sample'
    capture assert missing(`stored') == missing(`work') if `sample'
    local validation_rc = _rc
    if `validation_rc' {
        capture drop `stored'
        if `validation_rc' == 1 exit 1
        di as error "prediction is not representable in the requested storage type"
        exit 430
    }
    nobreak rename `stored' `varlist'
end

program define _fereg_p_xb
    version 18.0
    syntax newvarname [if] [in], STORage(string)
    // Plain numeric coefficient names need no factor or time-series expansion.
    // Score current data; no fitted-value cache survives data edits or sorting.
    // The route follows the caller's requested storage type, as before.
    local plain = inlist("`storage'", "float", "double")
    local columns : colnames e(b)
    foreach column of local columns {
        capture confirm name `column'
        if _rc local plain 0
    }
    if `plain' {
        tempname coefficients
        matrix `coefficients' = e(b)
        quietly matrix score `typlist' `varlist' = `coefficients' `if' `in'
    }
    else {
        quietly _predict `typlist' `varlist' `if' `in', xb
    }
end
