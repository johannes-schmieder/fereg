*! fereg public OLS dispatcher 0.6.5 19nov2026
program define _fereg_estimate_public, eclass
    version 15.0
    local public_cmdline `"fereg `0'"'
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         KEEPsingletons RESIDuals(name) SAVEFEs(string asis) ///
         SOLVer(string asis) THREADS(integer 0) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 16000) DETERMINISTIC TIMEIT VERBose(integer 0) ///
         NOCONStant]

    if `threads' < 0 | `poolsize' <= 0 | `tolerance' <= 0 | ///
       `tolerance' >= 1 | `iterate' <= 0 {
        di as error "threads(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    local memory_mode = lower(strtrim(`"`memory'"'))
    if `"`memory_mode'"' == "" local memory_mode "auto"
    if !inlist(`"`memory_mode'"', "auto", "fast", "low") {
        di as error "memory() must be auto, fast, or low"
        exit 198
    }
    // solver() holds the method, reduction, preconditioner and MAP
    // acceleration; _fereg_solver maps it to the native route tokens.
    _fereg_solver, command(fereg) spec(`"`solver'"')
    local solver_spec `"`r(spec)'"'
    local solver_mode "`r(route)'"
    local preconditioner_mode "`r(preconditioner)'"
    local reduction_option
    if "`solver_mode'" == "pcg" local reduction_option "reduction(`r(reduction)')"
    local cmgdegree `"`r(cmgdegree)'"'
    local cmgsamples `"`r(cmgsamples)'"'
    local cmgseed `"`r(cmgseed)'"'
    local cmgdegree_option
    if `"`solver_mode'"' == "cmgschur" {
        if `"`cmgdegree'"' == "" local cmgdegree 5
        if !inlist(`"`cmgdegree'"', "4", "5", "6") {
            di as error "cmgdegree() must be 4, 5, or 6"
            exit 198
        }
        local cmgdegree_option "cmgdegree(`cmgdegree')"
    }
    local cmgsamples_explicit = (`"`cmgsamples'"' != "")
    local cmgseed_explicit = (`"`cmgseed'"' != "")
    foreach option in cmgsamples cmgseed {
        if `"``option''"' != "" {
            local maximum "4294967295"
            if "`option'" == "cmgseed" local maximum "9007199254740991"
            if !regexm(strtrim(`"``option''"'), "^[0-9]+$") {
                di as error "`option'() must be an integer in 0..`maximum'"
                exit 198
            }
            if real(`"``option''"') > `maximum' {
                di as error "`option'() must be an integer in 0..`maximum'"
                exit 198
            }
            * Keep the exact decimal integer through the native request wire.
            local `option' = strtrim(string(real(`"``option''"'), "%21.0f"))
        }
        else local `option' 0
    }
    if `cmgseed_explicit' & `cmgsamples' == 0 {
        di as error "cmgseed() requires positive cmgsamples()"
        exit 198
    }
    if strtrim(`"`savefes'"') != "" {
        di as error "name saved fixed effects inside absorb(), for example absorb(fe=firm)"
        exit 198
    }
    if `"`weight'"' == "iweight" {
        di as error "iweights are not supported by the pinned reghdfe OLS contract"
        exit 101
    }

    local absorb_option
    local vce_option
    local cluster_option
    local dof_option
    local singleton_option
    local residual_option
    local constant_option
    local weight_option
    local fit_if `"`if'"'
    local memory_option `"memory(`memory_mode')"'
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    local vce_text = strtrim(`"`vce'"')
    if `"`vce_text'"' != "" {
        gettoken vce_kind vce_rest : vce_text
        local vce_kind = lower(`"`vce_kind'"')
        if `"`vce_kind'"' == "cluster" {
            local vce_rest = strtrim(`"`vce_rest'"')
            if `"`vce_rest'"' == "" {
                di as error "vce(cluster) requires at least one cluster dimension"
                exit 198
            }
            local cluster_option `"cluster(`vce_rest')"'
        }
        else if inlist(`"`vce_kind'"', "robust", "unadjusted", "ols", "iid") & ///
                strtrim(`"`vce_rest'"') == "" {
            local vce_option `"vce(`vce_kind')"'
        }
        else {
            di as error "vce() must be unadjusted, robust, or cluster clustervars"
            exit 198
        }
    }
    if strtrim(`"`dof'"') != "" local dof_option `"dof(`dof')"'
    if `"`keepsingletons'"' != "" local singleton_option "keepsingletons"
    if `"`residuals'"' != "" local residual_option `"residual(`residuals')"'
    if `"`noconstant'"' != "" local constant_option "noconstant"
    local public_wexp_option
    if `"`weight'"' != "" {
        local weight_variable = strtrim(subinstr(`"`exp'"', "=", "", 1))
        local weight_words : word count `weight_variable'
        capture confirm numeric variable `weight_variable'
        if _rc | `weight_words' != 1 {
            * Weight expressions are evaluated once on the if/in candidates,
            * as feppml does; e(wexp) keeps the caller's expression.
            tempvar public_weight_touse public_weight
            quietly mark `public_weight_touse' `if' `in'
            // Reuse the candidate decision: an if expression may be random or
            // stateful and must not be evaluated again by the hidden frontend.
            if `"`if'"' != "" local fit_if "if `public_weight_touse'"
            quietly generate double `public_weight' = ///
                `weight_variable' if `public_weight_touse'
            local weight_variable `public_weight'
            local public_wexp_option `"publicwexp(`"`exp'"')"'
        }
        if `"`weight'"' == "aweight" local weight_option "aweight(`weight_variable')"
        else if `"`weight'"' == "fweight" local weight_option "fweight(`weight_variable')"
        else if `"`weight'"' == "pweight" local weight_option "pweight(`weight_variable')"
    }

    local cmgsampling_options
    if `cmgsamples_explicit' local cmgsampling_options "cmgsamples(`cmgsamples')"
    if `cmgseed_explicit' local cmgsampling_options ///
        "`cmgsampling_options' cmgseed(`cmgseed')"

    local timeit_option
    if `"`timeit'"' != "" local timeit_option "publictimeit"
    _fereg_estimate_ols `varlist' `fit_if' `in', ///
        `absorb_option' `vce_option' `cluster_option' `dof_option' `singleton_option' ///
        `residual_option' `constant_option' `memory_option' ///
        `weight_option' `public_wexp_option' ///
        solver(`solver_mode') `reduction_option' `cmgdegree_option' `cmgsampling_options' ///
        preconditioner(`preconditioner_mode') ///
        poolsize(`poolsize') threads(`threads') tolerance(`tolerance') ///
        iterate(`iterate') `deterministic' ///
        publicmode publiccmdline(`"`public_cmdline'"') ///
        publicsolver(`"`solver_spec'"') publicverbose(`verbose') `timeit_option'

    if `"`timeit'"' != "" {
        local selected_memory `"`e(fereg_memory_selected)'"'
        di as txt "fereg timing (seconds): preflight=" ///
            as result %9.4f el(e(fereg_runtime), 18, 2) ///
            as txt " input=" as result %9.4f el(e(fereg_runtime), 19, 2) ///
            as txt " native=" as result %9.4f el(e(fereg_runtime), 20, 2) ///
            as txt " total-to-precommit=" as result %9.4f el(e(fereg_runtime), 21, 2)
        di as txt "fereg memory: requested=" as result "`memory_mode'" ///
            as txt " selected=" as result "`selected_memory'" ///
            as txt " native-peak-allocation=" ///
            as result %21.0fc el(e(fereg_runtime), 13, 2) ///
            as txt " scratch=" as result %21.0fc el(e(fereg_runtime), 9, 2)
        di as txt "fereg action memory: partial estimate; combined peak unavailable"
        di as txt "fereg execution: requested-threads=" ///
            as result %9.0f el(e(fereg_runtime), 28, 2) ///
            as txt " effective-threads=" as result %9.0f el(e(fereg_runtime), 29, 2) ///
            as txt " deterministic=" as result %1.0f el(e(fereg_runtime), 30, 2)
    }
end
