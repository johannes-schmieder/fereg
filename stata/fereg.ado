*! fereg 0.5.0 beta OLS 24oct2026
program define fereg, rclass
    version 18.0

    if substr(strtrim(`"`0'"'), 1, 1) == "," {
        syntax , SELFTEST [THREADS(integer 0) VERBOSE]

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

        return local action_protocol "1.6"
        return local model_wire "2.1"
        return local plugin_name "`plugin_name'"
        return local plugin_file `"`plugin_file'"'
        return scalar requested_threads = `threads'
        exit
    }

    local display_results = c(noisily)
    _fereg_estimate_public `0'
    if `display_results' {
        di as txt "Fixed-effect solver: " as result "`e(fereg_solver_used)'" ///
            as txt "   Residual check: " as result "`e(fereg_solver_residual)'"
        if e(fereg_solver_fallback) ///
            di as txt "A certified fallback was used; see e(fereg_solver)."
        ereturn display
    }
end
