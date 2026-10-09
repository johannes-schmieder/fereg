*! fereg public OLS syntax and result adapter 0.5.0 24aug2026
program define _fereg_ols, eclass sortpreserve
    version 18.0
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         KEEPsingletons RESIDuals(name) XB(name) XBD(name) D(name) NOCONStant ///
         SOLVer(string) PREConditioner(string) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 16000) THREADS(integer 0) DETERMINISTIC]

    local memory_mode = lower(strtrim(`"`memory'"'))
    if `"`memory_mode'"' == "" local memory_mode "fast"
    if !inlist(`"`memory_mode'"', "auto", "fast", "low") | `threads' < 0 | ///
       `poolsize' <= 0 | `tolerance' <= 0 | `tolerance' >= 1 | `iterate' <= 0 {
        di as error "memory(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    local weight_option
    if `"`weight'"' != "" {
        local weight_variable = strtrim(subinstr(`"`exp'"', "=", "", 1))
        if `"`weight'"' == "aweight" local weight_option "aweight(`weight_variable')"
        else if `"`weight'"' == "fweight" local weight_option "fweight(`weight_variable')"
        else if `"`weight'"' == "pweight" local weight_option "pweight(`weight_variable')"
        else if `"`weight'"' == "iweight" {
            di as error "iweights are not supported by the pinned reghdfe OLS contract"
            exit 101
        }
        else {
            di as error "hidden numeric manifest adapter received an unsupported weight"
            exit 498
        }
    }

    local vce_option
    local cluster_option
    local vce_text = strtrim(`"`vce'"')
    if `"`vce_text'"' != "" {
        gettoken vce_kind vce_rest : vce_text
        * Keywords are case-insensitive; cluster variable names keep their case.
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
            di as error "hidden numeric manifest adapter received an unsupported vce()"
            exit 498
        }
    }

    local absorb_option
    local dof_option
    local singleton_option
    local residual_option
    local constant_option
    local xb_option
    local xbd_option
    local d_option
    local memory_option `"memory(`memory_mode')"'
    local solver_option
    local preconditioner_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`dof'"') != "" local dof_option `"dof(`dof')"'
    if `"`keepsingletons'"' != "" local singleton_option "keepsingletons"
    if `"`residuals'"' != "" local residual_option `"residual(`residuals')"'
    if `"`xb'"' != "" local xb_option `"xb(`xb')"'
    if `"`xbd'"' != "" local xbd_option `"xbd(`xbd')"'
    if `"`d'"' != "" local d_option `"d(`d')"'
    if `"`noconstant'"' != "" local constant_option "noconstant"
    if strtrim(`"`solver'"') != "" local solver_option `"solver(`solver')"'
    if strtrim(`"`preconditioner'"') != "" ///
        local preconditioner_option `"preconditioner(`preconditioner')"'

    _fereg_estimate_ols `varlist' `if' `in', ///
        `absorb_option' `vce_option' `cluster_option' `dof_option' ///
        `singleton_option' `residual_option' `xb_option' `xbd_option' ///
        `d_option' `constant_option' `weight_option' `memory_option' ///
        `solver_option' `preconditioner_option' ///
        poolsize(`poolsize') threads(`threads') tolerance(`tolerance') ///
        iterate(`iterate') `deterministic'

    local coefficient_count = e(fereg_receipt)[1,5]
    local posted_indepvars
    if `coefficient_count' > 0 {
        local posted_indepvars : colfullnames e(b)
    }
    _fereg_indepvars, target(intended_indepvars) count(`coefficient_count') ///
        status(e(fereg_status)) names(`"`posted_indepvars'"')
    ereturn scalar fereg_v_determinant = e(fereg_certificate)[31,2]
    ereturn local indepvars `"`intended_indepvars'"'
    ereturn local cmd "_fereg_ols"
    ereturn local cmdline `"_fereg_ols `0'"'
    ereturn local fereg_gate "public_ols_v1"
    ereturn local fereg_intended_public_command "fereg"
end
