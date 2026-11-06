*! fereg 0.5.1 OLS 06nov2026
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
            _fereg_display, `diopts'
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

        di as txt "fereg.ado_version=0.5.1"
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

        return local ado_version "0.5.1"
        return local fereg_version "0.5.1"

        return local action_protocol "1.7"
        return local model_wire "2.1"
        return local plugin_name "`plugin_name'"
        return local plugin_file `"`plugin_file'"'
        return scalar requested_threads = `threads'
        exit
    }

    local display_results = c(noisily)
    _fereg_estimate_public `0'
    if `display_results' _fereg_display
end
