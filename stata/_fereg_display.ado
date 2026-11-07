*! Shared OLS/PPML result display 0.5.3 07nov2026
program define _fereg_display
    version 15.0
    syntax [, *]
    // Prefix commands own their inference and may omit the original fit metadata.
    if `"`e(prefix)'"' != "" {
        ereturn display, `options'
        exit
    }
    if `"`e(model)'"' == "poisson" {
        DisplayPpmlHeader
        di
        ereturn display, `options'
        DisplayAbsorbed
        exit
    }
    if `"`e(fereg_solver_used)'"' != "" {
        di as txt "Fixed-effect solver: " as result "`e(fereg_solver_used)'" _continue
        // Posted results passed the accuracy check; stop codes 2 and 5 record
        // a certified absorption (1 means there was nothing to absorb).
        tempname solver
        capture matrix `solver' = e(fereg_solver)
        if !_rc & e(fereg_tolerance) < . {
            if inlist(`solver'[2,4], 2, 5) {
                local tolerance : display %9.1e e(fereg_tolerance)
                local tolerance = subinstr(strtrim(`"`tolerance'"'), ".0e", "e", 1)
                di as txt "   Accuracy check: " as result "passed" ///
                    as txt " (tolerance " as result "`tolerance'" as txt ")" _continue
            }
        }
        di
        if e(fereg_solver_fallback) == 1 ///
            di as txt "A certified fallback was used; see e(fereg_solver)."
    }
    DisplayHeader
    di
    ereturn display, `options'
    DisplayAbsorbed
end

program define DisplayHeader
    local left1 `"`e(title)'"'
    if e(N_hdfe) > 0 & e(N_hdfe) < . {
        local plural = cond(e(N_hdfe) == 1, "", "s")
        local left2 "Absorbing `=e(N_hdfe)' HDFE group`plural'"
    }
    if inlist(`"`e(vce)'"', "robust", "cluster") ///
        local left3 "Statistics robust to heteroskedasticity"

    local clusters = cond(missing(e(N_clustervars)), 0, e(N_clustervars))
    local rows = max(7, 4 + `clusters')
    di
    forvalues row = 1/`rows' {
        local cluster = `row' - (`rows' - `clusters')
        if `cluster' > 0 {
            local label "Number of clusters (`e(clustvar`cluster')')"
            // Long interaction names get their own line, without truncation.
            if strlen(`"`label'"') > 36 {
                di as txt `"`label' = "' as result %10.0fc e(N_clust`cluster')
            }
            else {
                di as txt `"`label'"' _col(38) "= " ///
                    as result %10.0fc e(N_clust`cluster') _continue
            }
        }
        else di as txt `"`left`row''"' _continue

        if `row' == 1 ///
            di as txt _col(51) "Number of obs" _col(67) "= " as result %10.0fc e(N)
        else if `row' == 2 ///
            di as txt _col(51) "F(" as result %4.0f e(df_m) as txt "," ///
                as result %7.0f e(df_r) as txt ")" _col(67) "= " as result %10.2f e(F)
        else if `row' == 3 ///
            di as txt _col(51) "Prob > F" _col(67) "= " ///
                as result %10.4f Ftail(e(df_m), e(df_r), e(F))
        else if `row' == 4 ///
            di as txt _col(51) "R-squared" _col(67) "= " as result %10.4f e(r2)
        else if `row' == 5 ///
            di as txt _col(51) "Adj R-squared" _col(67) "= " as result %10.4f e(r2_a)
        else if `row' == 6 ///
            di as txt _col(51) "Within R-sq." _col(67) "= " as result %10.4f e(r2_within)
        else if `row' == 7 ///
            di as txt _col(51) "Root MSE" _col(67) "= " as result %10.4f e(rmse)
        else di
    }
end

program define DisplayPpmlHeader
    if e(converged) == 1 & e(ic) < . {
        local iterations = strtrim(string(e(ic), "%12.0g"))
        local plural = cond(e(ic) == 1, "", "s")
        di as txt "(PPML estimator converged in " as result "`iterations'" ///
            as txt " iteration`plural')"
    }
    di
    di as txt "HDFE PPML regression" _col(51) "Number of obs" ///
        _col(67) "= " as result %10.0fc e(N)
    if e(N_hdfe) > 0 & e(N_hdfe) < . {
        local plural = cond(e(N_hdfe) == 1, "", "s")
        di as txt "Absorbing `=e(N_hdfe)' HDFE group`plural'" _continue
    }
    di as txt _col(51) "Residual df" _col(67) "= " as result %10.0fc e(df_resid)
    local df = cond(missing(e(df_chi2)), e(df_m), e(df_chi2))
    di as txt "Statistics robust to heteroskedasticity" ///
        _col(51) "Wald chi2(" as result `df' as txt ")" ///
        _col(67) "= " as result %10.2f e(chi2)
    di as txt "Deviance" _col(22) "= " as result %12.0g e(deviance) ///
        as txt _col(51) "Prob > chi2" _col(67) "= " ///
        as result %10.4f chi2tail(`df', e(chi2))
    di as txt "Log pseudolikelihood = " as result %12.0g e(ll) ///
        as txt _col(51) "Pseudo R2" _col(67) "= " as result %10.4f e(r2_p)
    local clusters = cond(missing(e(N_clustervars)), 0, e(N_clustervars))
    if `clusters' > 0 di
    forvalues i = 1/`clusters' {
        di as txt "Number of clusters (`e(clustvar`i')') = " ///
            as result %10.0fc e(N_clust`i')
    }
end

program define DisplayAbsorbed
    if missing(e(N_hdfe)) | e(N_hdfe) == 0 exit
    tempname dof
    capture matrix `dof' = e(dof_table)
    if _rc exit
    local names : rownames `dof'
    local width 13
    foreach name of local names {
        local name = subinstr(`"`name'"', "1.", "", .)
        local width = max(`width', strlen(`"`name'"') + 1)
    }
    local rule = `width' + 40
    di as txt _n "Absorbed degrees of freedom:"
    di as txt "{hline `rule'}{c TRC}"
    di as txt %`width's "Absorbed FE " "{c |} Categories  - Redundant  = Num. Coefs {c |}"
    di as txt "{hline `width'}{c +}{hline 39}{c |}"
    local inexact 0
    local nested 0
    forvalues row = 1/`=rowsof(`dof')' {
        local name : word `row' of `names'
        local name = subinstr(`"`name'"', "1.", "", .)
        local note " "
        if `dof'[`row',4] == 1 {
            local note "?"
            local inexact 1
        }
        if `dof'[`row',5] == 1 {
            local note "*"
            local nested 1
        }
        di as txt %`width's "`name' " "{c |}" ///
            %10.0g `dof'[`row',1] "  " %10.0g `dof'[`row',2] "  " ///
            as result %10.0g `dof'[`row',3] as txt "  `note'  {c |}"
    }
    di as txt "{hline `width'}{c BT}{hline 39}{c BRC}"
    if `inexact' di as txt "? = number of redundant parameters may be higher"
    if `nested' di as txt "* = FE nested within cluster; treated as redundant for DoF computation"
end
