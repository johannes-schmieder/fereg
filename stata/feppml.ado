*! version 0.5.0 31oct2026
program define feppml, eclass
    version 15.0
    if replay() {
        syntax [, *]
        _get_diopts diopts options, `options'
        if `"`options'"' != "" {
            di as error `"option `options' not allowed"'
            exit 198
        }
        if `"`e(cmd)'"' != "feppml" error 301
        ereturn display, `diopts'
        exit
    }
    local host_processors = c(processors)
    local quiet_prefix
    if !c(noisily) local quiet_prefix quietly
    // Parsing and private output preparation can activate Stata's MP workers.
    // Keep the complete public call serial in the host; Rust owns its team.
    nobreak {
        if `host_processors' > 1 quietly set processors 1
        capture noisily break `quiet_prefix' _fereg_ppml `0'
        local fit_rc = _rc
        if `host_processors' > 1 quietly set processors `host_processors'
    }
    if `fit_rc' exit `fit_rc'
end
