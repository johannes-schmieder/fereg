/* Expanded Stata 19 comparison: 1/2/3 FEs, K=0/2/10/30, and native threads through 16. */
version 19.0
clear all
set more off
set linesize 255

args repo_root output_dir requested_threads scale warm_reps comparator_root design controls rep_offset preflight_spec arm_spec mode_spec cold_spec data_path
if `"`repo_root'"' == "" | `"`output_dir'"' == "" | `"`scale'"' == "" | `"`design'"' == "" {
    display as error "repo_root, output_dir, scale, and design are required"
    exit 198
}
if `"`requested_threads'"' == "" local requested_threads 1
if `"`warm_reps'"' == "" local warm_reps 5
if `"`controls'"' == "" local controls 2
if `"`rep_offset'"' == "" local rep_offset 0
if `"`preflight_spec'"' == "" local preflight_spec "on"
if `"`arm_spec'"' == "" local arm_spec "all"
if `"`mode_spec'"' == "" local mode_spec "both"
if `"`cold_spec'"' == "" local cold_spec "on"
if !inlist(`requested_threads', 1, 2, 4, 8, 16) | !inlist(`controls', 0, 2, 10, 30) {
    display as error "requested threads or control count is outside the V2 contract"
    exit 198
}
if !inlist(`"`preflight_spec'"', "on", "off") | !inlist(`"`cold_spec'"', "on", "off") {
    display as error "preflight and cold must be on or off"
    exit 198
}
if !inlist(`"`arm_spec'"', "all", "fereg", "greg", "reghdfe", "none") {
    display as error "unknown V2 arm"
    exit 198
}
if !inlist(`"`mode_spec'"', "both", "fit", "save_fe", "none") {
    display as error "unknown V2 output mode"
    exit 198
}
local stata_processors = min(`requested_threads', c(processors_lic), c(processors_mach))
set processors `stata_processors'
capture mkdir `"`output_dir'"'
if !inlist(`"`comparator_root'"', "", "system") sysdir set PLUS `"`comparator_root'"'
adopath ++ `"`repo_root'/stata"'
adopath ++ `"`repo_root'/dist"'
global GTOOLS_BETA "I KNOW WHAT I AM DOING"

tempname environment
file open `environment' using `"`output_dir'/environment.txt"', write text replace
file write `environment' "stata_version=`c(stata_version)'" _n
file write `environment' "stata_flavor=`c(flavor)'" _n
file write `environment' "os=`c(os)'" _n
file write `environment' "machine_type=`c(machine_type)'" _n
file write `environment' "processors_lic=`c(processors_lic)'" _n
file write `environment' "processors_mach=`c(processors_mach)'" _n
file write `environment' "requested_threads=`requested_threads'" _n
file write `environment' "stata_processors=`stata_processors'" _n
file write `environment' `"scale=`scale'"' _n
file write `environment' `"design=`design'"' _n
file write `environment' "controls=`controls'" _n
file write `environment' "warm_repetitions=`warm_reps'" _n
file write `environment' "rep_offset=`rep_offset'" _n
file write `environment' `"preflight=`preflight_spec'"' _n
file write `environment' `"arm=`arm_spec'"' _n
file write `environment' `"mode=`mode_spec'"' _n
file write `environment' `"cold=`cold_spec'"' _n
file write `environment' `"data_path=`data_path'"' _n
foreach command in fereg greg gregress reghdfe {
    capture findfile `command'.ado
    if _rc {
        file close `environment'
        display as error "required V2 command is unavailable: `command'"
        exit _rc
    }
    file write `environment' `"dependency_`command'=`r(fn)'"' _n
}
file close `environment'

capture program drop __fcc2_cleanup
program define __fcc2_cleanup
    capture drop __fcc2_*
end

capture program drop __fcc2_generate
program define __fcc2_generate
    args n workers firms periods mobility seed
    clear
    set rng mt64
    set seed `seed'
    quietly set obs `workers'
    generate long worker = _n
    quietly expand `periods'
    bysort worker: generate int period = _n
    generate int year = 2000 + period - 1
    bysort worker (period): generate long firm = mod(worker - 1, `firms') + 1 if _n == 1
    bysort worker (period): replace firm = cond(runiform() < `mobility', runiformint(1, `firms'), firm[_n-1]) if _n > 1
    bysort worker: generate double __worker_fe = rnormal(0, .4) if _n == 1
    bysort worker: replace __worker_fe = __worker_fe[1]
    bysort firm: generate double __firm_fe = rnormal(0, .2) if _n == 1
    bysort firm: replace __firm_fe = __firm_fe[1]
    bysort year: generate double __year_fe = rnormal(0, .1) if _n == 1
    bysort year: replace __year_fe = __year_fe[1]
    forvalues j = 1/30 {
        generate double x`j' = rnormal()
    }
    generate double y = 2 + __worker_fe + __firm_fe + __year_fe + .2*x1 - .1*x2 + rnormal()
    drop __worker_fe __firm_fe __year_fe
    assert _N == `n'
    foreach group in worker firm year {
        bysort `group': generate long __count = _N
        quietly summarize __count, meanonly
        assert r(min) >= 2
        drop __count
    }
    sort worker period
end

capture program drop __fcc2_preflight
program define __fcc2_preflight
    args absorb xvars case_id scale design controls fe_dimensions requested_threads stata_processors
    set processors `stata_processors'
    __fcc2_cleanup

    quietly reghdfe y `xvars', absorb(`absorb') dof(pairwise) ///
        transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
        tolerance(1e-10) iterate(16000) residuals(__fcc2_ref_res)
    matrix __fcc2_ref_b = e(b)
    matrix __fcc2_ref_v = e(V)
    scalar __fcc2_ref_n = e(N)
    scalar __fcc2_ref_rss = e(rss)
    scalar __fcc2_ref_dfa = e(df_a)

    quietly fereg y `xvars', absorb(`absorb') dof(pairwise) solver(auto) ///
        threads(`requested_threads') tolerance(1e-10) iterate(16000) residuals(__fcc2_f_res)
    matrix __fcc2_f_b = e(b)
    matrix __fcc2_f_v = e(V)
    matrix __fcc2_f_solver = e(fereg_solver)
    scalar __fcc2_f_n = e(N)
    scalar __fcc2_f_rss = e(rss)
    scalar __fcc2_f_dfa = e(df_a)
    mata: st_numscalar("__fcc2_f_bmax", max(abs(st_matrix("__fcc2_ref_b") :- st_matrix("__fcc2_f_b"))))
    mata: st_numscalar("__fcc2_f_bscale", max(abs(st_matrix("__fcc2_ref_b"))))
    mata: st_numscalar("__fcc2_f_vmax", max(abs(st_matrix("__fcc2_ref_v") :- st_matrix("__fcc2_f_v"))))
    mata: st_numscalar("__fcc2_f_vscale", max(abs(st_matrix("__fcc2_ref_v"))))
    scalar __fcc2_f_bscale = max(__fcc2_f_bscale, 1)
    scalar __fcc2_f_vscale = max(__fcc2_f_vscale, 1)
    generate double __fcc2_diff = abs(__fcc2_ref_res - __fcc2_f_res)
    quietly summarize __fcc2_diff, meanonly
    scalar __fcc2_f_resmax = r(max)
    drop __fcc2_diff
    scalar __fcc2_f_rssrel = abs(__fcc2_ref_rss - __fcc2_f_rss) / max(abs(__fcc2_ref_rss), 1)

    quietly greg y `xvars', absorb(`absorb') algorithm(cg) hdfetol(1e-10) ///
        maxiter(16000) mata(GtoolsRegress) resid(__fcc2_g_res)
    scalar __fcc2_g_n = r(N)
    scalar __fcc2_g_bmax = 0
    if `controls' > 0 {
        mata: __fcc2_g_ref = st_matrix("__fcc2_ref_b")
        mata: __fcc2_g_max = 0
        forvalues j = 1/`controls' {
            mata: __fcc2_g_max = max((__fcc2_g_max, abs(__fcc2_g_ref[1, `j'] - GtoolsRegress.b[1, `j'])))
        }
        mata: st_numscalar("__fcc2_g_bmax", __fcc2_g_max)
        mata: mata drop __fcc2_g_ref __fcc2_g_max
    }
    generate double __fcc2_diff = abs(__fcc2_ref_res - __fcc2_g_res)
    quietly summarize __fcc2_diff, meanonly
    scalar __fcc2_g_resmax = r(max)
    drop __fcc2_diff

    __fcc2_cleanup
    local f_absorb ""
    local r_absorb ""
    local g_alphas ""
    local f_sum "0"
    local r_sum "0"
    local g_sum "0"
    foreach effect of local absorb {
        local f_absorb `"`f_absorb' __fcc2_f_`effect'=`effect'"'
        local r_absorb `"`r_absorb' __fcc2_r_`effect'=`effect'"'
        local g_alphas `"`g_alphas' __fcc2_g_`effect'"'
        local f_sum `"`f_sum' + __fcc2_f_`effect'"'
        local r_sum `"`r_sum' + __fcc2_r_`effect'"'
        local g_sum `"`g_sum' + __fcc2_g_`effect'"'
    }
    quietly fereg y `xvars', absorb(`f_absorb') dof(pairwise) solver(auto) ///
        threads(`requested_threads') tolerance(1e-10) iterate(16000) residuals(__fcc2_f_res)
    predict double __fcc2_f_xb, xb
    generate double __fcc2_f_pred = __fcc2_f_xb + `f_sum'
    generate double __fcc2_diff = abs((y - __fcc2_f_res) - __fcc2_f_pred)
    quietly summarize __fcc2_diff, meanonly
    scalar __fcc2_f_reconmax = r(max)
    drop __fcc2_diff

    quietly reghdfe y `xvars', absorb(`r_absorb') dof(pairwise) ///
        transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
        tolerance(1e-10) iterate(16000) residuals(__fcc2_r_res)
    predict double __fcc2_r_xb, xb
    generate double __fcc2_r_pred = __fcc2_r_xb + `r_sum'
    generate double __fcc2_diff = abs((y - __fcc2_r_res) - __fcc2_r_pred)
    quietly summarize __fcc2_diff, meanonly
    scalar __fcc2_r_reconmax = r(max)
    drop __fcc2_diff

    quietly greg y `xvars', absorb(`absorb') algorithm(cg) hdfetol(1e-10) ///
        maxiter(16000) mata(GtoolsRegress) alphas(`g_alphas') savecons replace
    mata: st_numscalar("__fcc2_g_cons", GtoolsRegress.consest)
    generate double __fcc2_g_pred = __fcc2_g_cons + `g_sum'
    if `controls' > 0 {
        forvalues j = 1/`controls' {
            mata: st_numscalar("__fcc2_g_beta", GtoolsRegress.b[1, `j'])
            replace __fcc2_g_pred = __fcc2_g_pred + __fcc2_g_beta*x`j'
        }
    }
    generate double __fcc2_diff = abs(__fcc2_f_pred - __fcc2_r_pred)
    quietly summarize __fcc2_diff, meanonly
    scalar __fcc2_fr_predmax = r(max)
    replace __fcc2_diff = __fcc2_f_pred - __fcc2_g_pred
    quietly summarize __fcc2_diff, meanonly
    scalar __fcc2_fg_shift = r(mean)
    replace __fcc2_diff = abs(__fcc2_diff - __fcc2_fg_shift)
    quietly summarize __fcc2_diff, meanonly
    scalar __fcc2_fg_predmax = r(max)
    quietly summarize __fcc2_f_pred, meanonly
    scalar __fcc2_predscale = max(abs(r(min)), abs(r(max)), 1)

    local science_fail = 0
    if __fcc2_f_n != __fcc2_ref_n | __fcc2_g_n != __fcc2_ref_n local science_fail = 1
    if __fcc2_f_bmax > 1e-10 + 1e-7*__fcc2_f_bscale local science_fail = 1
    if __fcc2_g_bmax > 1e-10 + 1e-7*__fcc2_f_bscale local science_fail = 1
    if __fcc2_f_vmax > 1e-10 + 1e-6*__fcc2_f_vscale local science_fail = 1
    if __fcc2_f_rssrel > 1e-7 | __fcc2_f_dfa != __fcc2_ref_dfa local science_fail = 1
    if __fcc2_f_reconmax > 1e-10 + 1e-10*__fcc2_predscale local science_fail = 1

    post __fcc2_prepost (`"`case_id'"') (`"`scale'"') (`"`design'"') ///
        (`controls') (`fe_dimensions') (`requested_threads') (`stata_processors') ///
        (__fcc2_ref_n) (__fcc2_ref_dfa) (__fcc2_f_solver[1,2]) ///
        (__fcc2_f_bmax) (__fcc2_f_vmax) (__fcc2_f_rssrel) (__fcc2_f_resmax) ///
        (__fcc2_g_bmax) (__fcc2_g_resmax) (__fcc2_f_reconmax) ///
        (__fcc2_r_reconmax) (__fcc2_fr_predmax) (__fcc2_fg_predmax) (`science_fail')
    display as result "PREFLIGHT_V2|case=`case_id'|requested=`requested_threads'|stata=`stata_processors'|science_fail=`science_fail'"
    __fcc2_cleanup
    if `science_fail' exit 459
end

capture program drop __fcc2_time_arm
program define __fcc2_time_arm
    args absorb xvars case_id scale design controls fe_dimensions requested_threads stata_processors mode phase rep sequence arm
    set processors `stata_processors'
    __fcc2_cleanup
    local f_absorb `"`absorb'"'
    local r_absorb `"`absorb'"'
    local g_options ""
    if `"`mode'"' == "save_fe" {
        local f_absorb ""
        local r_absorb ""
        local g_alphas ""
        foreach effect of local absorb {
            local f_absorb `"`f_absorb' __fcc2_f_`effect'=`effect'"'
            local r_absorb `"`r_absorb' __fcc2_r_`effect'=`effect'"'
            local g_alphas `"`g_alphas' __fcc2_g_`effect'"'
        }
        local g_options `"alphas(`g_alphas') savecons replace"'
    }
    timer clear 91
    timer on 91
    if `"`arm'"' == "fereg" {
        capture quietly fereg y `xvars', absorb(`f_absorb') dof(pairwise) ///
            solver(auto) threads(`requested_threads') tolerance(1e-8) iterate(16000)
    }
    else if `"`arm'"' == "reghdfe" {
        capture quietly reghdfe y `xvars', absorb(`r_absorb') dof(pairwise) ///
            transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
            tolerance(1e-8) iterate(16000)
    }
    else if `"`arm'"' == "greg" {
        capture quietly greg y `xvars', absorb(`absorb') algorithm(cg) ///
            hdfetol(1e-8) maxiter(16000) mata(GtoolsRegress) `g_options'
    }
    else {
        display as error "unknown V2 arm"
        exit 198
    }
    local command_rc = _rc
    timer off 91
    quietly timer list 91
    scalar __fcc2_seconds = r(t91)
    timer clear 91
    scalar __fcc2_n = .
    scalar __fcc2_rss = .
    scalar __fcc2_dfa = .
    scalar __fcc2_requested = .
    scalar __fcc2_used = .
    scalar __fcc2_iterations = .
    scalar __fcc2_fallback = .
    scalar __fcc2_native = .
    scalar __fcc2_precommit = .
    scalar __fcc2_effective = .
    if `command_rc' == 0 & `"`arm'"' == "greg" scalar __fcc2_n = r(N)
    else if `command_rc' == 0 {
        scalar __fcc2_n = e(N)
        scalar __fcc2_rss = e(rss)
        scalar __fcc2_dfa = e(df_a)
    }
    if `command_rc' == 0 & `"`arm'"' == "fereg" {
        matrix __fcc2_solver = e(fereg_solver)
        matrix __fcc2_runtime = e(fereg_runtime)
        scalar __fcc2_requested = __fcc2_solver[1,1]
        scalar __fcc2_used = __fcc2_solver[1,2]
        scalar __fcc2_iterations = __fcc2_solver[1,3]
        scalar __fcc2_fallback = __fcc2_solver[1,11]
        scalar __fcc2_native = __fcc2_runtime[20,2]
        scalar __fcc2_precommit = __fcc2_runtime[21,2]
        scalar __fcc2_effective = __fcc2_runtime[29,2]
    }
    post __fcc2_runpost (`"`case_id'"') (`"`scale'"') (`"`design'"') ///
        (`controls') (`fe_dimensions') (`requested_threads') (`stata_processors') ///
        (`"`mode'"') (`"`phase'"') (`rep') (`sequence') (`"`arm'"') ///
        (`command_rc') (__fcc2_seconds) (__fcc2_n) (__fcc2_rss) (__fcc2_dfa) ///
        (__fcc2_requested) (__fcc2_used) (__fcc2_iterations) (__fcc2_fallback) ///
        (__fcc2_native) (__fcc2_precommit) (__fcc2_effective)
    display as result "RUN_V2|case=`case_id'|requested=`requested_threads'|stata=`stata_processors'|mode=`mode'|phase=`phase'|rep=`rep'|arm=`arm'|rc=`command_rc'|seconds=" __fcc2_seconds
    if `command_rc' exit `command_rc'
end

postfile __fcc2_runpost str48 case_id str8 scale str24 design int controls ///
    byte fe_dimensions int requested_threads int stata_processors str8 mode ///
    str5 phase int rep int sequence str10 arm int rc double seconds n rss df_a ///
    requested_solver used_solver iterations fallback native_seconds ///
    precommit_seconds fereg_effective_threads ///
    using `"`output_dir'/command_comparison_v2_runs.dta"', replace
postfile __fcc2_prepost str48 case_id str8 scale str24 design int controls ///
    byte fe_dimensions int requested_threads int stata_processors double n df_a ///
    fereg_solver fereg_bmax fereg_vmax fereg_rssrel fereg_resmax greg_bmax ///
    greg_resmax fereg_reconstruct_max reghdfe_reconstruct_max ///
    fereg_reghdfe_predmax fereg_greg_predmax byte science_fail ///
    using `"`output_dir'/command_comparison_v2_preflights.dta"', replace

if `"`scale'"' == "n1m" {
    local n 1000000
    local workers 100000
    local firms 5000
}
else if `"`scale'"' == "n10m" {
    local n 10000000
    local workers 1000000
    local firms 50000
}
else {
    display as error "unknown V2 scale"
    exit 198
}
if `"`design'"' == "one_fe" {
    local absorb "worker"
    local mobility .15
    local seed 2026082704
    local fe_dimensions 1
}
else if inlist(`"`design'"', "akm_standard", "akm_standard_year") {
    local absorb "worker firm"
    if `"`design'"' == "akm_standard_year" local absorb "worker firm year"
    local mobility .15
    local seed 2026082704
    local fe_dimensions 2
    if `"`design'"' == "akm_standard_year" local fe_dimensions 3
}
else if inlist(`"`design'"', "akm_lowmob", "akm_lowmob_year") {
    local absorb "worker firm"
    if `"`design'"' == "akm_lowmob_year" local absorb "worker firm year"
    local mobility .02
    local seed 2026082705
    local fe_dimensions 2
    if `"`design'"' == "akm_lowmob_year" local fe_dimensions 3
}
else {
    display as error "unknown V2 design"
    exit 198
}
local xvars ""
if `controls' > 0 {
    forvalues j = 1/`controls' {
        local xvars `"`xvars' x`j'"'
    }
}
local case_id "`design'_`scale'_k`controls'"
capture confirm file `"`data_path'"'
if _rc {
    display as text "GENERATE_V2|case=`case_id'|path=`data_path'"
    __fcc2_generate `n' `workers' `firms' 10 `mobility' `seed'
    quietly save `"`data_path'"', replace
}
else {
    display as text "LOAD_V2|case=`case_id'|path=`data_path'"
    quietly use `"`data_path'"', clear
    assert _N == `n'
    confirm variable worker firm year x1 x30 y
}
if `"`preflight_spec'"' == "on" {
    __fcc2_preflight `"`absorb'"' `"`xvars'"' `case_id' `scale' `design' ///
        `controls' `fe_dimensions' `requested_threads' `stata_processors'
}
local modes "fit save_fe"
if inlist(`"`mode_spec'"', "fit", "save_fe") local modes `"`mode_spec'"'
if `"`mode_spec'"' == "none" local modes ""
local mode_index = 0
foreach mode of local modes {
    local ++mode_index
    local arms "fereg greg reghdfe"
    if `"`arm_spec'"' != "all" {
        if `"`arm_spec'"' == "none" local arms ""
        else local arms `"`arm_spec'"'
    }
    local arm_count : word count `arms'
    if `arm_count' > 0 & `"`cold_spec'"' == "on" {
        local shift = mod(`mode_index' + `requested_threads' + `controls', `arm_count')
        forvalues sequence = 1/`arm_count' {
            local arm_index = mod(`sequence' + `shift' - 1, `arm_count') + 1
            local arm : word `arm_index' of `arms'
            __fcc2_time_arm `"`absorb'"' `"`xvars'"' `case_id' `scale' `design' ///
                `controls' `fe_dimensions' `requested_threads' `stata_processors' ///
                `mode' cold 0 `sequence' `arm'
        }
    }
    if `arm_count' > 0 & `warm_reps' > 0 {
        forvalues rep = 1/`warm_reps' {
            local global_rep = `rep' + `rep_offset'
            local shift = mod(`mode_index' + `requested_threads' + `controls' + `global_rep', `arm_count')
            forvalues sequence = 1/`arm_count' {
                local arm_index = mod(`sequence' + `shift' - 1, `arm_count') + 1
                local arm : word `arm_index' of `arms'
                __fcc2_time_arm `"`absorb'"' `"`xvars'"' `case_id' `scale' `design' ///
                    `controls' `fe_dimensions' `requested_threads' `stata_processors' ///
                    `mode' warm `global_rep' `sequence' `arm'
            }
        }
    }
}
postclose __fcc2_runpost
postclose __fcc2_prepost
use `"`output_dir'/command_comparison_v2_runs.dta"', clear
sort scale design controls requested_threads mode phase rep arm
export delimited using `"`output_dir'/command_comparison_v2_runs.csv"', replace
use `"`output_dir'/command_comparison_v2_preflights.dta"', clear
sort scale design controls requested_threads
export delimited using `"`output_dir'/command_comparison_v2_preflights.csv"', replace
tempname success
file open `success' using `"`output_dir'/SUCCESS.txt"', write text replace
file write `success' "status=success" _n
file write `success' "completed_date=`c(current_date)'" _n
file write `success' "completed_time=`c(current_time)'" _n
file close `success'
display as result "END_COMMAND_COMPARISON_V2|rc=0"
exit 0
