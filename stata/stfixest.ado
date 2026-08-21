*! StFixest 0.1.0 Phase 0-1 20aug2026
program define stfixest, rclass
    version 18.0
    syntax , [VERSION DOCTOR SELFTEST THREADS(integer 0) VERBOSE]

    local actions = (`"`version'"' != "") + (`"`doctor'"' != "") + (`"`selftest'"' != "")
    if `actions' != 1 {
        di as error "specify exactly one of version, doctor, or selftest"
        exit 198
    }
    if `threads' < 0 {
        di as error "threads() must be nonnegative; threads(0) selects the native default"
        exit 198
    }

    quietly _stfixest_load
    local handle "`r(handle)'"
    local plugin_name "`r(plugin_name)'"
    local plugin_file `"`r(plugin_file)'"'

    if `"`version'"' != "" {
        plugin call `handle', version
    }
    else if `"`doctor'"' != "" {
        plugin call `handle', doctor ///
            ado=0.1.0-phase01 ///
            protocol_major=1 ///
            protocol_minor=0 ///
            stata=`c(stata_version)' ///
            threads=`threads' ///
            `"plugin_file=`plugin_file'"' ///
            dependency_audit=build-time-required
        if `"`verbose'"' != "" {
            di as txt "Stata OS: `c(os)'"
            di as txt "Stata machine type: `c(machine_type)'"
            capture di as txt "Stata processors available: `c(processors)'"
            capture di as txt "Stata processors licensed: `c(processors_lic)'"
            capture di as txt "Stata processors detected: `c(processors_mach)'"
        }
    }
    else {
        plugin call `handle', selftest ///
            ado=0.1.0-phase01 ///
            protocol_major=1 ///
            protocol_minor=0 ///
            stata=`c(stata_version)' ///
            threads=`threads'
    }

    return local ado_version "0.1.0-phase01"
    return local model_protocol "1.0"
    return local plugin_name "`plugin_name'"
    return local plugin_file `"`plugin_file'"'
    return scalar requested_threads = `threads'
end
