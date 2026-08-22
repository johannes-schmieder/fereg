*! StFixest developer-only OLS numeric-manifest adapter 0.1.0 22aug2026
program define _stfixest_ols_hidden_numeric_v1, eclass sortpreserve
    version 18.0
    syntax varlist(fv numeric min=1) [if] [in] [fw aw pw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         KEEPsingletons RESIDuals(name) NOCONStant]

    local weight_option
    if `"`weight'"' != "" {
        local weight_variable = strtrim(subinstr(`"`exp'"', "=", "", 1))
        if `"`weight'"' == "aweight" local weight_option "aweight(`weight_variable')"
        else if `"`weight'"' == "fweight" local weight_option "fweight(`weight_variable')"
        else if `"`weight'"' == "pweight" local weight_option "pweight(`weight_variable')"
        else {
            di as error "hidden numeric manifest adapter received an unsupported weight"
            exit 498
        }
    }

    local vce_option
    local cluster_option
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

    local absorb_option
    local dof_option
    local singleton_option
    local residual_option
    local constant_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`dof'"') != "" local dof_option `"dof(`dof')"'
    if `"`keepsingletons'"' != "" local singleton_option "keepsingletons"
    if `"`residuals'"' != "" local residual_option `"residual(`residuals')"'
    if `"`noconstant'"' != "" local constant_option "noconstant"

    _stfixest_estimate_ols_test `varlist' `if' `in', ///
        `absorb_option' `vce_option' `cluster_option' `dof_option' ///
        `singleton_option' `residual_option' `constant_option' `weight_option'

    local coefficient_count = e(stfixest_receipt)[1,5]
    local posted_indepvars
    if `coefficient_count' > 0 {
        local posted_indepvars : colfullnames e(b)
    }
    local intended_indepvars
    foreach name of local posted_indepvars {
        local name = subinstr(`"`name'"', "o.", "", 1)
        local intended_indepvars `"`intended_indepvars' `name'"'
    }
    local intended_indepvars = strtrim(`"`intended_indepvars'"')
    ereturn scalar stfixest_v_determinant = e(stfixest_certificate)[31,2]
    ereturn local indepvars `"`intended_indepvars'"'
    ereturn local cmd "_stfixest_ols_hidden_numeric_v1"
    ereturn local cmdline `"_stfixest_ols_hidden_numeric_v1 `0'"'
    ereturn local stfixest_gate "developer_hidden_numeric_v1"
    ereturn local stfixest_intended_public_command "stfeols"
end
