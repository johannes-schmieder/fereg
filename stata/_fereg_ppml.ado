*! fereg public PPML syntax and result adapter 0.5.0 02sep2026
program define _fereg_ppml, eclass sortpreserve
    version 18.0
    local public_display = c(noisily)
    local public_cmdline `"feppml `0'"'
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) KEEPsingletons ///
         OFFset(varname numeric) EXPosure(varname numeric) ///
         SEParation(string asis) ///
         RESIDuals(name) RESIDualtype(string) ///
         XB(name) D(name) ETA(name) XBD(name) MU(name) DROPreason(name) ///
         MEMory(string) POOLSize(integer 10) THREADS(integer 0) ///
         DETERMINISTIC ///
         SOLVer(string) ///
         TOLerance(real 1e-8) ITERate(integer 10000) ///
         TIMEIT]

    local solver_mode = lower(strtrim(`"`solver'"'))
    if `"`solver_mode'"' == "" local solver_mode "auto"
    if !inlist(`"`solver_mode'"', "auto", "map", "lsmr", "cmg") {
        di as error "solver() must be auto, map, lsmr, or cmg"
        exit 198
    }

    if inlist(`"`weight'"', "aweight", "iweight") {
        di as error "feppml supports only fweights and pweights"
        exit 101
    }
    if `threads' < 0 | `poolsize' <= 0 | `tolerance' <= 0 | ///
       `tolerance' >= 1 | `iterate' <= 0 {
        di as error "invalid PPML execution, tolerance, or iteration control"
        exit 198
    }
    if `"`offset'"' != "" & `"`exposure'"' != "" {
        di as error "offset() and exposure() are mutually exclusive"
        exit 198
    }
    local weight_option
    local public_wexp_option
    if `"`weight'"' != "" {
        local weight_expression = strtrim(substr(`"`exp'"', 2, .))
        tempvar public_weight_touse
        quietly mark `public_weight_touse' `if' `in'
        tempvar public_weight
        quietly generate double `public_weight' = ///
            `weight_expression' if `public_weight_touse'
        local weight_variable `public_weight'
        if `"`weight'"' == "fweight" local weight_option "fweight(`weight_variable')"
        if `"`weight'"' == "pweight" local weight_option "pweight(`weight_variable')"
        local public_wexp_option `"publicwexp(`"`exp'"')"'
    }
    local absorb_option
    local vce_option
    local singleton_option
    local offset_option
    local exposure_option
    local separation_option
    local residual_option
    local residual_type_option
    local xb_option
    local d_option
    local eta_option
    local xbd_option
    local mu_option
    local drop_option
    local memory_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`vce'"') != "" local vce_option `"vce(`vce')"'
    if `"`keepsingletons'"' != "" local singleton_option "keepsingletons"
    if `"`offset'"' != "" local offset_option "offset(`offset')"
    if `"`exposure'"' != "" local exposure_option "exposure(`exposure')"
    if strtrim(`"`separation'"') != "" local separation_option `"separation(`separation')"'
    if `"`residuals'"' != "" local residual_option "residual(`residuals')"
    if strtrim(`"`residualtype'"') != "" ///
        local residual_type_option "residualtype(`residualtype')"
    if `"`xb'"' != "" local xb_option "xb(`xb')"
    if `"`d'"' != "" local d_option "d(`d')"
    if `"`eta'"' != "" local eta_option "eta(`eta')"
    if `"`xbd'"' != "" local xbd_option "xbd(`xbd')"
    if `"`mu'"' != "" local mu_option "mu(`mu')"
    if `"`dropreason'"' != "" local drop_option "dropreason(`dropreason')"
    if strtrim(`"`memory'"') != "" local memory_option "memory(`memory')"
    local timeit_option
    if `"`timeit'"' != "" local timeit_option "publictimeit"
    local display_option
    if `public_display' local display_option "publicdisplay"

    _fereg_estimate_ppml `varlist' `if' `in', ///
        `weight_option' `absorb_option' `vce_option' `singleton_option' ///
        `offset_option' `exposure_option' `separation_option' ///
        `residual_option' `residual_type_option' `xb_option' `d_option' ///
        `eta_option' `xbd_option' `mu_option' `drop_option' ///
        `memory_option' solver(`solver_mode') poolsize(`poolsize') threads(`threads') ///
        tolerance(`tolerance') iterate(`iterate') ///
        septolerance(1e-8) sepiterate(10000) ///
        simplexiterate(100000) simplexcells(1000000) ///
        `deterministic' publicmode publiccmdline(`"`public_cmdline'"') ///
        `public_wexp_option' `timeit_option' `display_option'
end
