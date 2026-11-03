*! fereg 0.5.0 OLS 03nov2026
program define fereg, rclass
    version 15.0

    if replay() {
        syntax [, SELFTEST THREADS(integer 0) VERBOSE *]
        if `"`selftest'"' == "" {
            if `threads' != 0 | `"`verbose'"' != "" {
                di as error "threads() and verbose are valid only with selftest"
                exit 198
            }
            _get_diopts diopts options, `options'
            if `"`options'"' != "" {
                di as error `"option `options' not allowed"'
                exit 198
            }
            if `"`e(cmd)'"' != "fereg" error 301
            Display, `diopts'
            exit
        }
        if `"`options'"' != "" {
            di as error `"option `options' not allowed"'
            exit 198
        }

        if `threads' < 0 {
            di as error "threads() must be nonnegative; threads(0) selects the native default"
            exit 198
        }

        di as txt "fereg.ado_version=0.5.0"
        if `"`verbose'"' != "" {
            di as txt "Stata OS: `c(os)'"
            di as txt "Stata machine type: `c(machine_type)'"
            capture di as txt "Stata processors available: `c(processors)'"
            capture di as txt "Stata processors licensed: `c(processors_lic)'"
            capture di as txt "Stata processors detected: `c(processors_mach)'"
        }

        _fereg_load, action(selftest) threads(`threads') `verbose'
        local plugin_name "`r(plugin_name)'"
        local plugin_file `"`r(plugin_file)'"'

        return local ado_version "0.5.0"
        return local fereg_version "0.5.0"

        return local action_protocol "1.7"
        return local model_wire "2.1"
        return local plugin_name "`plugin_name'"
        return local plugin_file `"`plugin_file'"'
        return scalar requested_threads = `threads'
        exit
    }

    local display_results = c(noisily)
    _fereg_estimate_public `0'
    if `display_results' Display
end

program define Display
    syntax [, *]
    // Prefix commands own their inference and may omit the original fit metadata.
    if `"`e(prefix)'"' != "" {
        ereturn display, `options'
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
