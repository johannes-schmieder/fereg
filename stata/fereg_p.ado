*! fereg prediction adapter 0.9.0 24aug2026
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

    marksample touse, novarlist
    if `"`xb'"' != "" {
        quietly _predict `typlist' `varlist' if `touse' & e(sample), xb
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
        quietly generate `typlist' `varlist' = `residual_var' if `touse' & e(sample)
        exit
    }
    if `"`xbd'"' != "" {
        quietly generate `typlist' `varlist' = `e(depvar)' - `residual_var' ///
            if `touse' & e(sample)
        exit
    }

    tempvar linear_prediction
    quietly _predict double `linear_prediction' if `touse' & e(sample), xb
    quietly generate `typlist' `varlist' = ///
        `e(depvar)' - `linear_prediction' - `residual_var' ///
        if `touse' & e(sample)
end
