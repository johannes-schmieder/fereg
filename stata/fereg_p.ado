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
    if `"`xb'"' != "" {
        quietly _fereg_p_xb `typlist' `varlist' if `sample'
        exit
    }

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
        quietly generate `typlist' `varlist' = `residual_var' if `sample'
        exit
    }
    if `"`xbd'"' != "" {
        quietly generate `typlist' `varlist' = `e(depvar)' - `residual_var' ///
            if `sample'
        exit
    }

    tempvar linear_prediction
    quietly _fereg_p_xb double `linear_prediction' if `sample'
    quietly generate `typlist' `varlist' = ///
        `e(depvar)' - `linear_prediction' - `residual_var' ///
        if `sample'
end

program define _fereg_p_xb
    version 18.0
    syntax newvarname [if] [in]
    // Plain numeric coefficient names need no factor or time-series expansion.
    // Score current data; no fitted-value cache survives data edits or sorting.
    local plain = inlist("`typlist'", "float", "double")
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
