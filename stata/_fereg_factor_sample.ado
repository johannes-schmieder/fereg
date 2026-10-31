*! fereg factor expansion availability 0.5.0
program define _fereg_factor_sample, rclass
    version 15.0
    syntax varlist(fv ts numeric), TOUSE(varname numeric) GENerate(name) ///
        [ABSorb(string asis) CLUster(string asis) AWEIGHT(varname numeric) ///
         FWEIGHT(varname numeric) PWEIGHT(varname numeric) ///
         OFFset(varname numeric) EXPosure(varname numeric)]

    // Continuous-only models need no observation pass. Unexpanded factor
    // syntax is deliberately left to fvexpand for its authoritative parsing.
    local factors 0
    foreach term of local varlist {
        capture quietly _ms_parse_parts `term'
        if _rc local factors 1
        else if "`r(type)'" != "variable" local factors 1
    }
    if !`factors' {
        return local touse "`touse'"
        exit
    }

    // This mask controls expansion only. Rust still receives the original
    // candidate mask and owns drop reasons, domains and the final sample.
    quietly generate byte `generate' = `touse'
    markout `generate' `varlist'
    local weightvar `aweight'`fweight'`pweight'
    if "`weightvar'" != "" markout `generate' `weightvar'
    if "`aweight'" != "" quietly replace `generate' = 0 if `aweight' == 0
    local absorb_option
    local cluster_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`cluster'"') != "" local cluster_option `"cluster(`cluster')"'
    quietly fereg_parse, touse(`touse') emptymodel `absorb_option' `cluster_option'
    local required `"`r(source_vars)'"'
    if strtrim(`"`required'"') != "" markout `generate' `required', strok
    if "`offset'`exposure'" != "" markout `generate' `offset' `exposure'
    return local touse "`generate'"
end
