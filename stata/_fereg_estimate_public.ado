*! fereg public OLS dispatcher 0.5.0 24aug2026
program define _fereg_estimate_public, eclass
    version 18.0
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         KEEPsingletons RESIDuals(name) SAVEFEs(string asis) ///
         SOLVer(string) PREConditioner(string) THREADS(integer 0) ///
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
    local solver_mode = lower(strtrim(`"`solver'"'))
    if `"`solver_mode'"' == "" local solver_mode "auto"
    if `"`solver_mode'"' == "cg" {
        di as error "solver(cg) is disabled because its residual certificate does not ensure projection accuracy; use solver(lsmr) or solver(cmg)"
        exit 198
    }
    if !inlist(`"`solver_mode'"', "auto", "map", "lsmr", "cmg", "it", "squarem") {
        di as error "solver() must be auto, map, lsmr, cmg, it, or squarem"
        exit 198
    }
    local preconditioner_mode = lower(strtrim(`"`preconditioner'"'))
    if `"`preconditioner_mode'"' == "" {
        if !inlist(`"`solver_mode'"', "auto", "lsmr") local preconditioner_mode "off"
        else local preconditioner_mode "auto"
    }
    if `"`preconditioner_mode'"' == "none" local preconditioner_mode "off"
    if `"`preconditioner_mode'"' == "additive" local preconditioner_mode "schwarz"
    if !inlist(`"`preconditioner_mode'"', "auto", "off", "diagonal", "schwarz") {
        di as error "preconditioner() must be auto, off, diagonal, or schwarz"
        exit 198
    }
    if !inlist(`"`solver_mode'"', "auto", "lsmr") & `"`preconditioner_mode'"' != "off" {
        di as error "preconditioner() is valid only with solver(auto) or solver(lsmr)"
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
    local memory_option `"memory(`memory_mode')"'
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    local vce_text = lower(strtrim(`"`vce'"'))
    if `"`vce_text'"' != "" {
        gettoken vce_kind vce_rest : vce_text
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
            di as error "hidden numeric manifest adapter received an unsupported vce()"
            exit 498
        }
    }
    if strtrim(`"`dof'"') != "" local dof_option `"dof(`dof')"'
    if `"`keepsingletons'"' != "" local singleton_option "keepsingletons"
    if `"`residuals'"' != "" local residual_option `"residual(`residuals')"'
    if `"`noconstant'"' != "" local constant_option "noconstant"
    if `"`weight'"' != "" {
        local weight_variable = strtrim(subinstr(`"`exp'"', "=", "", 1))
        if `"`weight'"' == "aweight" local weight_option "aweight(`weight_variable')"
        else if `"`weight'"' == "fweight" local weight_option "fweight(`weight_variable')"
        else if `"`weight'"' == "pweight" local weight_option "pweight(`weight_variable')"
    }

    _fereg_estimate_ols `varlist' `if' `in', ///
        `absorb_option' `vce_option' `cluster_option' `dof_option' `singleton_option' ///
        `residual_option' `constant_option' `memory_option' ///
        `weight_option' ///
        solver(`solver_mode') preconditioner(`preconditioner_mode') ///
        poolsize(`poolsize') threads(`threads') tolerance(`tolerance') ///
        iterate(`iterate') `deterministic'

    // Preserve the public coefficient-axis adapter without paying for a
    // second full syntax/parser program invocation on every estimate.
    local coefficient_count = e(fereg_receipt)[1,5]
    local posted_indepvars
    if `coefficient_count' > 0 local posted_indepvars : colfullnames e(b)
    local intended_indepvars
    if `coefficient_count' > 0 {
        forvalues slot = 1/`coefficient_count' {
            if e(fereg_status)[`slot',2] == 2 continue
            local name : word `slot' of `posted_indepvars'
            local remaining `"`name'"'
            local intended_name
            while `"`remaining'"' != "" {
                gettoken component remaining : remaining, parse("#")
                if `"`component'"' == "#" {
                    local intended_name `"`intended_name'#"'
                    continue
                }
                local intended_component `"`component'"'
                if substr(`"`component'"', 1, 2) == "o." {
                    local intended_component = substr(`"`component'"', 3, .)
                }
                else if regexm(`"`component'"', "^(-?[0-9]+)(b|bn|o)?[.](.+)$") {
                    local factor_level = regexs(1)
                    local factor_variable = regexs(3)
                    local intended_component `"`factor_level'bn.`factor_variable'"'
                }
                local intended_name `"`intended_name'`intended_component'"'
            }
            local intended_indepvars `"`intended_indepvars' `intended_name'"'
        }
    }
    local intended_indepvars = strtrim(`"`intended_indepvars'"')
    ereturn scalar fereg_v_determinant = e(fereg_certificate)[31,2]
    ereturn local indepvars `"`intended_indepvars'"'
    ereturn local fereg_gate "public_ols_v1"
    ereturn local fereg_intended_public_command "fereg"

    ereturn local cmd "fereg"
    ereturn local cmdline `"fereg `0'"'
    ereturn local predict "fereg_p"
    local selected_memory_code = el(e(fereg_runtime), 2, 2)
    local selected_memory = cond(`selected_memory_code' == 1, "fast", "low")
    ereturn local fereg_memory_mode "`memory_mode'"
    ereturn local fereg_memory_requested "`memory_mode'"
    ereturn local fereg_memory_selected "`selected_memory'"
    ereturn local fereg_solver_requested "`solver_mode'"
    ereturn local fereg_preconditioner "`preconditioner_mode'"
    ereturn scalar fereg_requested_threads = el(e(fereg_runtime), 28, 2)
    ereturn scalar fereg_threads = el(e(fereg_runtime), 29, 2)
    ereturn scalar fereg_poolsize = `poolsize'
    ereturn scalar fereg_requested_tolerance = `tolerance'
    ereturn scalar fereg_tolerance = el(e(fereg_runtime), 23, 2)
    ereturn scalar fereg_iterate = `iterate'
    ereturn scalar fereg_deterministic = el(e(fereg_runtime), 30, 2)
    ereturn scalar fereg_timeit = (`"`timeit'"' != "")
    ereturn scalar fereg_verbose = `verbose'
    ereturn scalar fereg_memory_fast_bytes = el(e(fereg_runtime), 4, 2)
    ereturn scalar fereg_memory_low_bytes = el(e(fereg_runtime), 5, 2)
    ereturn scalar fereg_memory_selected_bytes = el(e(fereg_runtime), 25, 2)
    ereturn scalar fereg_scratch_bytes = el(e(fereg_runtime), 9, 2)
    ereturn scalar fereg_native_allocation_bytes = el(e(fereg_runtime), 13, 2)
    if el(e(fereg_runtime), 6, 1) == 1 ///
        ereturn scalar fereg_available_memory_bytes = el(e(fereg_runtime), 6, 2)
    if el(e(fereg_runtime), 15, 1) == 1 ///
        ereturn scalar fereg_rss_bytes = el(e(fereg_runtime), 15, 2)
    if el(e(fereg_runtime), 17, 1) == 1 ///
        ereturn scalar fereg_peak_rss_bytes = el(e(fereg_runtime), 17, 2)
    if `"`timeit'"' != "" {
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
