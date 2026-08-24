*! fereg 0.1.0 Phase 0-1 24aug2026
program define fereg, rclass sortpreserve
    version 18.0

    if substr(strtrim(`"`0'"'), 1, 1) == "," {
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

        if `"`version'"' != "" {
            local action "version"
        }
        else if `"`doctor'"' != "" {
            local action "doctor"
        }
        else {
            local action "selftest"
        }

        _fereg_load, action(`action') threads(`threads')
        local handle "`r(handle)'"
        local plugin_name "`r(plugin_name)'"
        local plugin_file `"`r(plugin_file)'"'

        if `"`doctor'"' != "" & `"`verbose'"' != "" {
            di as txt "Stata OS: `c(os)'"
            di as txt "Stata machine type: `c(machine_type)'"
            capture di as txt "Stata processors available: `c(processors)'"
            capture di as txt "Stata processors licensed: `c(processors_lic)'"
            capture di as txt "Stata processors detected: `c(processors_mach)'"
        }

        return local ado_version "0.1.0-phase01"
        return local action_protocol "1.0"
        return local model_wire "2.1"
        return local plugin_name "`plugin_name'"
        return local plugin_file `"`plugin_file'"'
        return scalar requested_threads = `threads'
        exit
    }

    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         KEEPsingletons RESIDuals(name) SAVEFEs(string asis) ///
         SOLVer(string) PREConditioner(string) THREADS(integer 0) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 16000) DETERMINISTIC TIMEIT VERBose(integer 0) ///
         NOCONStant]

    marksample touse, novarlist
    gettoken depvar indepvars : varlist

    if `threads' < 0 | `poolsize' <= 0 | `tolerance' <= 0 | `iterate' <= 0 {
        di as error "threads(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    di as error "fereg estimation is not enabled in fereg 0.1.0 Phase 0-1"
    di as error "This package proves the Rust-first plugin, protocol, parser, and safety architecture only."
    di as error "No partial or uncertified OLS/WLS estimate has been posted."
    exit 498
end
