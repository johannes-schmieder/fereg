*! fereg 0.5.0 beta OLS 31oct2026
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
        return local fereg_version "0.5.0-beta.12"

        return local action_protocol "1.6"
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
    // After bootstrap: or jackknife:, the fereg-specific results may be absent.
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
    ereturn display, `options'
end
