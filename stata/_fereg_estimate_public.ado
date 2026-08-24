*! fereg public OLS dispatcher 0.1.0 24aug2026
program define _fereg_estimate_public, eclass sortpreserve
    version 18.0
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         KEEPsingletons RESIDuals(name) SAVEFEs(string asis) ///
         SOLVer(string) PREConditioner(string) THREADS(integer 0) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 16000) DETERMINISTIC TIMEIT VERBose(integer 0) ///
         NOCONStant]

    if `threads' < 0 | `poolsize' <= 0 | `tolerance' <= 0 | `iterate' <= 0 {
        di as error "threads(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    local memory_mode = lower(strtrim(`"`memory'"'))
    if `"`memory_mode'"' == "" local memory_mode "auto"
    if !inlist(`"`memory_mode'"', "auto", "fast", "low") {
        di as error "memory() must be auto, fast, or low"
        exit 198
    }
    local solver_mode = lower(strtrim(`"`solver'"'))
    if `"`solver_mode'"' == "" local solver_mode "map"
    if `"`solver_mode'"' != "map" {
        di as error "this release candidate supports solver(map)"
        exit 498
    }
    local preconditioner_mode = lower(strtrim(`"`preconditioner'"'))
    if `"`preconditioner_mode'"' == "" local preconditioner_mode "none"
    if `"`preconditioner_mode'"' != "none" {
        di as error "solver(map) supports preconditioner(none)"
        exit 498
    }
    if strtrim(`"`savefes'"') != "" {
        di as error "name saved fixed effects inside absorb(), for example absorb(fe=firm)"
        exit 198
    }

    local absorb_option
    local vce_option
    local dof_option
    local singleton_option
    local residual_option
    local constant_option
    local weight_clause
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`vce'"') != "" local vce_option `"vce(`vce')"'
    if strtrim(`"`dof'"') != "" local dof_option `"dof(`dof')"'
    if `"`keepsingletons'"' != "" local singleton_option "keepsingletons"
    if `"`residuals'"' != "" local residual_option `"residuals(`residuals')"'
    if `"`noconstant'"' != "" local constant_option "noconstant"
    if `"`weight'"' != "" local weight_clause `"[`weight'`exp']"'

    _fereg_ols `varlist' `if' `in' `weight_clause', ///
        `absorb_option' `vce_option' `dof_option' `singleton_option' ///
        `residual_option' `constant_option'

    ereturn local cmd "fereg"
    ereturn local cmdline `"fereg `0'"'
    ereturn local predict "fereg_p"
    ereturn local fereg_memory_mode "`memory_mode'"
    ereturn local fereg_solver "`solver_mode'"
    ereturn local fereg_preconditioner "`preconditioner_mode'"
    ereturn scalar fereg_requested_threads = `threads'
    ereturn scalar fereg_poolsize = `poolsize'
    ereturn scalar fereg_tolerance = `tolerance'
    ereturn scalar fereg_iterate = `iterate'
    ereturn scalar fereg_deterministic = (`"`deterministic'"' != "")
    ereturn scalar fereg_timeit = (`"`timeit'"' != "")
    ereturn scalar fereg_verbose = `verbose'
    ereturn display
end
