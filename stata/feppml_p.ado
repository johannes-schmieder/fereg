*! fereg PPML prediction adapter 0.5.0 31aug2026
program define feppml_p
    version 18.0
    if !inlist(`"`e(cmd)'"', "feppml", "_fereg_estimate_ppml") {
        error 301
    }
    syntax newvarname [if] [in] [, MU XB D ABSorbed ETA XBD ///
        RESponse SCORE PEARson WORKing DEViance ANScombe]

    if `"`d'"' != "" & `"`absorbed'"' != "" {
        di as error "d and absorbed are aliases and may not be combined"
        exit 198
    }
    if `"`eta'"' != "" & `"`xbd'"' != "" {
        di as error "eta and xbd are aliases and may not be combined"
        exit 198
    }
    local requested = (`"`mu'"' != "") + (`"`xb'"' != "") + ///
        (`"`d'"' != "") + (`"`absorbed'"' != "") + ///
        (`"`eta'"' != "") + (`"`xbd'"' != "") + ///
        (`"`response'"' != "") + (`"`score'"' != "") + ///
        (`"`pearson'"' != "") + (`"`working'"' != "") + ///
        (`"`deviance'"' != "") + (`"`anscombe'"' != "")
    if `requested' > 1 {
        di as error "only one PPML prediction statistic may be specified"
        exit 198
    }
    if `requested' == 0 local mu "mu"

    marksample touse, novarlist
    tempvar prediction_work mean_work eta_work relative_work deviance_power

    if `"`xb'"' != "" {
        quietly _predict double `eta_work' if `touse' & e(sample), xb nooffset
        if `"`e(offset)'"' != "" quietly replace `eta_work' = `eta_work' + `e(offset)' ///
            if `touse' & e(sample)
        if `"`e(exposure)'"' != "" quietly replace `eta_work' = `eta_work' + log(`e(exposure)') ///
            if `touse' & e(sample)
        quietly generate `typlist' `prediction_work' = `eta_work' if `touse' & e(sample)
        _fereg_ppml_prediction_validate `prediction_work', touse(`touse')
        nobreak rename `prediction_work' `varlist'
        exit
    }

    if `"`d'`absorbed'"' != "" {
        if e(N_hdfe) == 0 {
            quietly generate `typlist' `prediction_work' = 0 if `touse' & e(sample)
        }
        else {
            local stored `"`e(d_var)'"'
            if `"`stored'"' == "" {
                di as error "predict, d after absorb() requires d() at estimation time"
                exit 498
            }
            capture confirm numeric variable `stored'
            local stored_rc = _rc
            if `stored_rc' {
                if `stored_rc' == 1 exit 1
                di as error "the saved PPML absorbed-contribution variable is no longer available"
                exit 111
            }
            quietly generate `typlist' `prediction_work' = `stored' if `touse' & e(sample)
        }
        _fereg_ppml_prediction_validate `prediction_work', touse(`touse')
        nobreak rename `prediction_work' `varlist'
        exit
    }

    // A retained d() is preferred because it combines the current finite
    // covariates and adjustment with the absorbed snapshot.  Retained eta()
    // and mu() are self-contained bounded-memory snapshot fallbacks and must
    // not require current RHS or adjustment variables to remain available.
    if e(N_hdfe) > 0 {
        local stored_d `"`e(d_var)'"'
        if `"`stored_d'"' != "" {
            capture confirm numeric variable `stored_d'
            local stored_d_rc = _rc
            if `stored_d_rc' {
                if `stored_d_rc' == 1 exit 1
                di as error "the saved PPML absorbed-contribution variable is no longer available"
                exit 111
            }
            quietly _predict double `eta_work' if `touse' & e(sample), xb nooffset
            if `"`e(offset)'"' != "" quietly replace `eta_work' = ///
                `eta_work' + `e(offset)' if `touse' & e(sample)
            if `"`e(exposure)'"' != "" quietly replace `eta_work' = ///
                `eta_work' + log(`e(exposure)') if `touse' & e(sample)
            quietly replace `eta_work' = `eta_work' + `stored_d' if `touse' & e(sample)
        }
        else {
            local stored_eta `"`e(eta_var)'"'
            if `"`stored_eta'"' != "" {
                capture confirm numeric variable `stored_eta'
                local stored_eta_rc = _rc
                if `stored_eta_rc' {
                    if `stored_eta_rc' == 1 exit 1
                    di as error "the saved PPML eta variable is no longer available"
                    exit 111
                }
                quietly generate double `eta_work' = `stored_eta' if `touse' & e(sample)
            }
            else {
                local stored_mu `"`e(mu_var)'"'
                if `"`stored_mu'"' == "" {
                    di as error "this prediction after absorb() requires d(), eta(), or mu() at estimation time"
                    exit 498
                }
                capture confirm numeric variable `stored_mu'
                local stored_mu_rc = _rc
                if `stored_mu_rc' {
                    if `stored_mu_rc' == 1 exit 1
                    di as error "the saved PPML mean variable is no longer available"
                    exit 111
                }
                quietly generate double `eta_work' = log(`stored_mu') ///
                    if `touse' & e(sample)
            }
        }
    }
    else {
        quietly _predict double `eta_work' if `touse' & e(sample), xb nooffset
        if `"`e(offset)'"' != "" quietly replace `eta_work' = ///
            `eta_work' + `e(offset)' if `touse' & e(sample)
        if `"`e(exposure)'"' != "" quietly replace `eta_work' = ///
            `eta_work' + log(`e(exposure)') if `touse' & e(sample)
    }

    if `"`eta'`xbd'"' != "" {
        quietly generate `typlist' `prediction_work' = `eta_work' if `touse' & e(sample)
        _fereg_ppml_prediction_validate `prediction_work', touse(`touse')
        nobreak rename `prediction_work' `varlist'
        exit
    }

    quietly generate double `mean_work' = exp(`eta_work') if `touse' & e(sample)

    if `"`mu'"' != "" {
        quietly generate `typlist' `prediction_work' = `mean_work' if `touse' & e(sample)
        _fereg_ppml_prediction_validate `prediction_work', touse(`touse') positive
        nobreak rename `prediction_work' `varlist'
        exit
    }
    if `"`response'`score'"' != "" {
        quietly generate `typlist' `prediction_work' = `e(depvar)' - `mean_work' ///
            if `touse' & e(sample)
        _fereg_ppml_prediction_validate `prediction_work', touse(`touse')
        nobreak rename `prediction_work' `varlist'
        exit
    }
    if `"`pearson'"' != "" {
        quietly generate `typlist' `prediction_work' = ///
            (`e(depvar)' - `mean_work') / sqrt(`mean_work') ///
            if `touse' & e(sample)
        _fereg_ppml_prediction_validate `prediction_work', touse(`touse')
        nobreak rename `prediction_work' `varlist'
        exit
    }
    if `"`working'"' != "" {
        quietly generate `typlist' `prediction_work' = ///
            (`e(depvar)' - `mean_work') / `mean_work' ///
            if `touse' & e(sample)
        _fereg_ppml_prediction_validate `prediction_work', touse(`touse')
        nobreak rename `prediction_work' `varlist'
        exit
    }
    if `"`deviance'"' != "" {
        capture confirm numeric variable `e(depvar)'
        local depvar_rc = _rc
        if `depvar_rc' exit `depvar_rc'
        capture assert `e(depvar)' >= 0 if `touse' & e(sample) & `e(depvar)' < .
        local domain_rc = _rc
        if `domain_rc' {
            if `domain_rc' == 1 exit 1
            di as error "PPML deviance is undefined for a negative outcome"
            exit 430
        }
        quietly generate `typlist' `prediction_work' = 2 * `mean_work' ///
            if `touse' & e(sample)
        quietly generate double `relative_work' = ///
            (`e(depvar)' - `mean_work') / `mean_work' ///
            if `touse' & e(sample) & `e(depvar)' > 0
        // Match the native near-equality series so predict, deviance remains
        // nonnegative when y and mu differ by only a few floating-point ulps.
        quietly generate double `deviance_power' = `relative_work'^2 ///
            if `touse' & e(sample) & `e(depvar)' > 0 & ///
            abs(`relative_work') <= 1e-4
        quietly replace `prediction_work' = `mean_work' * `deviance_power' ///
            if `touse' & e(sample) & `e(depvar)' > 0 & ///
            abs(`relative_work') <= 1e-4
        forvalues order = 3/12 {
            local deviance_sign = cond(mod(`order', 2) == 0, 1, -1)
            local deviance_coefficient = ///
                2 * `deviance_sign' / (`order' * (`order' - 1))
            quietly replace `deviance_power' = ///
                `deviance_power' * `relative_work' ///
                if `touse' & e(sample) & `e(depvar)' > 0 & ///
                abs(`relative_work') <= 1e-4
            quietly replace `prediction_work' = `prediction_work' + ///
                `deviance_coefficient' * `mean_work' * `deviance_power' ///
                if `touse' & e(sample) & `e(depvar)' > 0 & ///
                abs(`relative_work') <= 1e-4
        }
        quietly replace `prediction_work' = 2 * (`e(depvar)' * ///
            (log(`e(depvar)') - log(`mean_work')) - ///
            (`e(depvar)' - `mean_work')) ///
            if `touse' & e(sample) & `e(depvar)' > 0 & ///
            abs(`relative_work') > 1e-4
        _fereg_ppml_prediction_validate `prediction_work', ///
            touse(`touse') nonnegative
        nobreak rename `prediction_work' `varlist'
        exit
    }
    quietly generate `typlist' `prediction_work' = 1.5 * ///
        (`e(depvar)'^(2/3) - `mean_work'^(2/3)) / `mean_work'^(1/6) ///
        if `touse' & e(sample)
    _fereg_ppml_prediction_validate `prediction_work', touse(`touse')
    nobreak rename `prediction_work' `varlist'
end

program define _fereg_ppml_prediction_validate
    version 18.0
    syntax varname(numeric), TOUSE(varname numeric) [POSitive NONNegative]

    local validity `"`varlist' < ."'
    if `"`positive'"' != "" local validity `"`validity' & `varlist' > 0"'
    if `"`nonnegative'"' != "" local validity `"`validity' & `varlist' >= 0"'
    capture assert `validity' if `touse' & e(sample)
    local validation_rc = _rc
    if `validation_rc' {
        capture drop `varlist'
        if `validation_rc' == 1 exit 1
        di as error "PPML prediction is not representable in the requested storage type"
        exit 430
    }
end
