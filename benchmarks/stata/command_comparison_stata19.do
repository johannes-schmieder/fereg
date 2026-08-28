/*
    Registered Stata 19 command comparison for fereg.

    Usage:
      do command_comparison_stata19.do REPO_ROOT OUTPUT_DIR [THREADS] [SCALES] [WARM_REPS] [AREG_SCOPE] [COMPARATOR_ROOT] [DESIGN] [REP_OFFSET] [PREFLIGHT] [ARM] [MODE] [COLD] [DATA_PATH]

    THREADS defaults to "1 8" and SCALES defaults to "n100k n1m n10m".
    AREG_SCOPE is auto, all, one_fe, or off.  auto selects all on Stata 19+
    and one_fe on Stata 18, where areg does not support multiple absorbed FEs.
    Data generation, preflights, cleanup, and result posting are outside timing.
*/

version 18.0
clear all
set more off
set linesize 255

args repo_root output_dir thread_spec scale_spec warm_reps areg_scope comparator_root design_spec rep_offset preflight_spec arm_spec mode_spec cold_spec data_path
if `"`repo_root'"' == "" {
    display as error "repo_root is required"
    exit 198
}
if `"`output_dir'"' == "" {
    display as error "output_dir is required"
    exit 198
}
if `"`thread_spec'"' == "" local thread_spec "1 8"
if `"`scale_spec'"' == "" local scale_spec "n100k n1m n10m"
if `"`warm_reps'"' == "" local warm_reps 5
if `"`areg_scope'"' == "" local areg_scope "auto"
if `"`design_spec'"' == "" local design_spec "all"
if `"`rep_offset'"' == "" local rep_offset 0
if `"`preflight_spec'"' == "" local preflight_spec "on"
if `"`arm_spec'"' == "" local arm_spec "all"
if `"`mode_spec'"' == "" local mode_spec "both"
if `"`cold_spec'"' == "" local cold_spec "on"
if `warm_reps' < 0 | `warm_reps' != floor(`warm_reps') {
    display as error "warm_reps must be a nonnegative integer"
    exit 198
}
if `rep_offset' < 0 | `rep_offset' != floor(`rep_offset') {
    display as error "rep_offset must be a nonnegative integer"
    exit 198
}
if `"`areg_scope'"' == "auto" {
    if real("`c(stata_version)'") >= 19 local areg_scope "all"
    else local areg_scope "one_fe"
}
if !inlist(`"`areg_scope'"', "all", "one_fe", "off") {
    display as error "areg_scope must be auto, all, one_fe, or off"
    exit 198
}
if `"`areg_scope'"' == "all" & real("`c(stata_version)'") < 19 {
    display as error "areg_scope(all) requires Stata 19 or newer"
    exit 198
}
if !inlist(`"`preflight_spec'"', "on", "off") {
    display as error "preflight must be on or off"
    exit 198
}
if !inlist(`"`arm_spec'"', "all", "fereg", "greg", "reghdfe", "areg", "none") {
    display as error "arm must be all, fereg, greg, reghdfe, areg, or none"
    exit 198
}
if !inlist(`"`mode_spec'"', "both", "fit", "save_fe", "none") {
    display as error "mode must be both, fit, save_fe, or none"
    exit 198
}
if !inlist(`"`cold_spec'"', "on", "off") {
    display as error "cold must be on or off"
    exit 198
}
if `"`arm_spec'"' == "areg" & `"`areg_scope'"' == "off" {
    display as error "arm(areg) requires areg_scope(all) or areg_scope(one_fe)"
    exit 198
}
local scale_count : word count `scale_spec'
if `"`data_path'"' != "" & (`scale_count' != 1 | `"`design_spec'"' == "all") {
    display as error "data_path requires exactly one scale and one design"
    exit 198
}

capture mkdir `"`output_dir'"'
if `"`comparator_root'"' != "" sysdir set PLUS `"`comparator_root'"'
adopath ++ `"`repo_root'/stata"'
adopath ++ `"`repo_root'/dist"'
global GTOOLS_BETA "I KNOW WHAT I AM DOING"

tempname environment
file open `environment' using `"`output_dir'/environment.txt"', write text replace
file write `environment' "stata_version=`c(stata_version)'" _n
file write `environment' "stata_flavor=`c(flavor)'" _n
file write `environment' "os=`c(os)'" _n
file write `environment' "machine_type=`c(machine_type)'" _n
file write `environment' "configured_processors=`c(processors)'" _n
file write `environment' "warm_repetitions=`warm_reps'" _n
file write `environment' `"threads=`thread_spec'"' _n
file write `environment' `"scales=`scale_spec'"' _n
file write `environment' `"areg_scope=`areg_scope'"' _n
file write `environment' `"comparator_root=`comparator_root'"' _n
file write `environment' `"designs=`design_spec'"' _n
file write `environment' "rep_offset=`rep_offset'" _n
file write `environment' `"preflight=`preflight_spec'"' _n
file write `environment' `"arm=`arm_spec'"' _n
file write `environment' `"mode=`mode_spec'"' _n
file write `environment' `"cold=`cold_spec'"' _n
file write `environment' `"data_path=`data_path'"' _n

local required_commands "fereg greg gregress reghdfe"
if `"`areg_scope'"' != "off" local required_commands "`required_commands' areg"
foreach command of local required_commands {
    capture findfile `command'.ado
    local dependency_rc = _rc
    if `dependency_rc' {
        file write `environment' "dependency_`command'_rc=`dependency_rc'" _n
        file close `environment'
        display as error "required command is unavailable: `command'"
        exit `dependency_rc'
    }
    file write `environment' `"dependency_`command'=`r(fn)'"' _n
}
file close `environment'

capture program drop __fcc_cleanup
program define __fcc_cleanup
    foreach variable in __fcc_f_worker __fcc_f_firm __fcc_r_worker __fcc_r_firm ///
        __fcc_g_worker __fcc_g_firm __fcc_ref_res __fcc_f_res __fcc_r_res ///
        __fcc_a_res __fcc_g_res __fcc_f_pred __fcc_r_pred __fcc_g_pred ///
        __fcc_diff {
        capture drop `variable'
    }
end

capture program drop __fcc_generate
program define __fcc_generate
    args n workers firms periods mobility seed
    clear
    set rng mt64
    set seed `seed'
    quietly set obs `workers'
    generate long worker = _n
    quietly expand `periods'
    bysort worker: generate int period = _n
    bysort worker (period): generate long firm = mod(worker - 1, `firms') + 1 if _n == 1
    bysort worker (period): replace firm = ///
        cond(runiform() < `mobility', runiformint(1, `firms'), firm[_n-1]) if _n > 1

    bysort worker: generate double __fcc_worker_fe = rnormal(0, .4) if _n == 1
    bysort worker: replace __fcc_worker_fe = __fcc_worker_fe[1]
    bysort firm: generate double __fcc_firm_fe = rnormal(0, .2) if _n == 1
    bysort firm: replace __fcc_firm_fe = __fcc_firm_fe[1]
    generate double x1 = rnormal()
    generate double x2 = rnormal()
    generate double y = 2 + __fcc_worker_fe + __fcc_firm_fe + .2*x1 - .1*x2 + rnormal()
    drop __fcc_worker_fe __fcc_firm_fe

    assert _N == `n'
    bysort worker: generate long __fcc_count = _N
    quietly summarize __fcc_count, meanonly
    assert r(min) >= 2
    drop __fcc_count
    bysort firm: generate long __fcc_count = _N
    quietly summarize __fcc_count, meanonly
    assert r(min) >= 2
    drop __fcc_count
    sort worker period
end

capture program drop __fcc_preflight
program define __fcc_preflight
    args case_id scale design absorb threads areg_enabled
    set processors `threads'
    __fcc_cleanup

    quietly reghdfe y x1 x2, absorb(`absorb') dof(pairwise) ///
        transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
        tolerance(1e-10) iterate(16000) residuals(__fcc_ref_res)
    matrix __fcc_ref_b = e(b)
    matrix __fcc_ref_v = e(V)
    scalar __fcc_ref_n = e(N)
    scalar __fcc_ref_rss = e(rss)
    scalar __fcc_ref_dfa = e(df_a)
    scalar __fcc_a_n = .
    scalar __fcc_a_dfa = .
    scalar __fcc_a_bmax = .
    scalar __fcc_a_vmax = .
    scalar __fcc_a_rssrel = .
    scalar __fcc_a_resmax = .

    quietly fereg y x1 x2, absorb(`absorb') dof(pairwise) solver(auto) ///
        threads(`threads') tolerance(1e-10) iterate(16000) residuals(__fcc_f_res)
    matrix __fcc_f_b = e(b)
    matrix __fcc_f_v = e(V)
    scalar __fcc_f_n = e(N)
    scalar __fcc_f_rss = e(rss)
    scalar __fcc_f_dfa = e(df_a)
    matrix __fcc_f_solver = e(fereg_solver)

    mata: st_numscalar("__fcc_f_bmax", max(abs(st_matrix("__fcc_ref_b") :- st_matrix("__fcc_f_b"))))
    mata: st_numscalar("__fcc_f_bscale", max(abs(st_matrix("__fcc_ref_b"))))
    mata: st_numscalar("__fcc_f_vmax", max(abs(st_matrix("__fcc_ref_v") :- st_matrix("__fcc_f_v"))))
    mata: st_numscalar("__fcc_f_vscale", max(abs(st_matrix("__fcc_ref_v"))))
    scalar __fcc_f_bscale = max(__fcc_f_bscale, 1)
    scalar __fcc_f_vscale = max(__fcc_f_vscale, 1)
    generate double __fcc_diff = abs(__fcc_ref_res - __fcc_f_res)
    quietly summarize __fcc_diff, meanonly
    scalar __fcc_f_resmax = r(max)
    drop __fcc_diff
    scalar __fcc_f_rssrel = abs(__fcc_ref_rss - __fcc_f_rss) / max(abs(__fcc_ref_rss), 1)

    if `areg_enabled' {
        if `"`absorb'"' == "worker" {
            quietly areg y x1 x2, absorb(worker)
        }
        else {
            quietly areg y x1 x2, absorb(`absorb') dfabsorb noabsorbtest ///
                tolerance(1e-10) iterate(16000) nolog
        }
        matrix __fcc_a_b = e(b)
        matrix __fcc_a_v = e(V)
        scalar __fcc_a_n = e(N)
        scalar __fcc_a_rss = e(rss)
        scalar __fcc_a_dfa = e(df_a)
        predict double __fcc_a_res, residuals
        mata: st_numscalar("__fcc_a_bmax", max(abs(st_matrix("__fcc_ref_b") :- st_matrix("__fcc_a_b"))))
        mata: st_numscalar("__fcc_a_vmax", max(abs(st_matrix("__fcc_ref_v") :- st_matrix("__fcc_a_v"))))
        generate double __fcc_diff = abs(__fcc_ref_res - __fcc_a_res)
        quietly summarize __fcc_diff, meanonly
        scalar __fcc_a_resmax = r(max)
        drop __fcc_diff
        scalar __fcc_a_rssrel = abs(__fcc_ref_rss - __fcc_a_rss) / max(abs(__fcc_ref_rss), 1)
    }

    quietly greg y x1 x2, absorb(`absorb') algorithm(cg) hdfetol(1e-10) ///
        maxiter(16000) mata(GtoolsRegress) resid(__fcc_g_res)
    scalar __fcc_g_n = r(N)
    mata: st_numscalar("__fcc_g_b1", GtoolsRegress.b[1,1])
    mata: st_numscalar("__fcc_g_b2", GtoolsRegress.b[1,2])
    scalar __fcc_g_bmax = max( ///
        abs(__fcc_ref_b[1,colnumb(__fcc_ref_b,"x1")] - __fcc_g_b1), ///
        abs(__fcc_ref_b[1,colnumb(__fcc_ref_b,"x2")] - __fcc_g_b2))
    generate double __fcc_diff = abs(__fcc_ref_res - __fcc_g_res)
    quietly summarize __fcc_diff, meanonly
    scalar __fcc_g_resmax = r(max)
    drop __fcc_diff

    /* The three commands that can publish component coefficients must also
       agree on their normalization-invariant fitted predictor. */
    __fcc_cleanup
    if `"`absorb'"' == "worker" {
        local f_absorb "__fcc_f_worker=worker"
        local r_absorb "__fcc_r_worker=worker"
        local g_alphas "__fcc_g_worker"
        local f_sum "__fcc_f_worker"
        local r_sum "__fcc_r_worker"
        local g_sum "__fcc_g_worker"
    }
    else {
        local f_absorb "__fcc_f_worker=worker __fcc_f_firm=firm"
        local r_absorb "__fcc_r_worker=worker __fcc_r_firm=firm"
        local g_alphas "__fcc_g_worker __fcc_g_firm"
        local f_sum "__fcc_f_worker + __fcc_f_firm"
        local r_sum "__fcc_r_worker + __fcc_r_firm"
        local g_sum "__fcc_g_worker + __fcc_g_firm"
    }
    quietly fereg y x1 x2, absorb(`f_absorb') dof(pairwise) solver(auto) ///
        threads(`threads') tolerance(1e-10) iterate(16000) residuals(__fcc_f_res)
    generate double __fcc_f_pred = _b[_cons] + _b[x1]*x1 + _b[x2]*x2 + `f_sum'
    generate double __fcc_diff = abs((y - __fcc_f_res) - __fcc_f_pred)
    quietly summarize __fcc_diff, meanonly
    scalar __fcc_f_reconmax = r(max)
    drop __fcc_diff
    quietly reghdfe y x1 x2, absorb(`r_absorb') dof(pairwise) ///
        transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
        tolerance(1e-10) iterate(16000) residuals(__fcc_r_res)
    generate double __fcc_r_pred = _b[_cons] + _b[x1]*x1 + _b[x2]*x2 + `r_sum'
    generate double __fcc_diff = abs((y - __fcc_r_res) - __fcc_r_pred)
    quietly summarize __fcc_diff, meanonly
    scalar __fcc_r_reconmax = r(max)
    drop __fcc_diff
    quietly greg y x1 x2, absorb(`absorb') algorithm(cg) hdfetol(1e-10) ///
        maxiter(16000) mata(GtoolsRegress) alphas(`g_alphas') savecons replace
    mata: st_numscalar("__fcc_g_cons", GtoolsRegress.consest)
    mata: st_numscalar("__fcc_g_save_b1", GtoolsRegress.b[1,1])
    mata: st_numscalar("__fcc_g_save_b2", GtoolsRegress.b[1,2])
    generate double __fcc_g_pred = __fcc_g_cons + __fcc_g_save_b1*x1 + ///
        __fcc_g_save_b2*x2 + `g_sum'
    generate double __fcc_diff = abs(__fcc_f_pred - __fcc_r_pred)
    quietly summarize __fcc_diff, meanonly
    scalar __fcc_fr_predmax = r(max)
    replace __fcc_diff = __fcc_f_pred - __fcc_g_pred
    quietly summarize __fcc_diff, meanonly
    scalar __fcc_fg_shift = r(mean)
    replace __fcc_diff = abs(__fcc_diff - __fcc_fg_shift)
    quietly summarize __fcc_diff, meanonly
    scalar __fcc_fg_predmax = r(max)
    quietly summarize __fcc_f_pred, meanonly
    scalar __fcc_predscale = max(abs(r(min)), abs(r(max)), 1)

    local science_fail = 0
    if __fcc_f_n != __fcc_ref_n | __fcc_g_n != __fcc_ref_n local science_fail = 1
    if `areg_enabled' & __fcc_a_n != __fcc_ref_n local science_fail = 1
    if __fcc_f_bmax > 1e-10 + 1e-7*__fcc_f_bscale local science_fail = 1
    if `areg_enabled' & __fcc_a_bmax > 1e-10 + 1e-7*__fcc_f_bscale local science_fail = 1
    if __fcc_g_bmax > 1e-10 + 1e-7*__fcc_f_bscale local science_fail = 1
    if __fcc_f_vmax > 1e-10 + 1e-6*__fcc_f_vscale local science_fail = 1
    if `areg_enabled' & __fcc_a_vmax > 1e-10 + 1e-6*__fcc_f_vscale local science_fail = 1
    if __fcc_f_rssrel > 1e-7 local science_fail = 1
    if `areg_enabled' & __fcc_a_rssrel > 1e-7 local science_fail = 1
    if __fcc_f_dfa != __fcc_ref_dfa local science_fail = 1
    if __fcc_f_reconmax > 1e-10 + 1e-10*__fcc_predscale local science_fail = 1
    /* greg remains a diagnostic arm.  Its beta saved-alpha interface has a
       distinct published normalization contract, so only N/slopes/residuals
       are hard-gated here; the predictor distance is retained for diagnosis. */

    post __fcc_prepost (`"`case_id'"') (`"`scale'"') (`"`design'"') (`threads') ///
        (__fcc_ref_n) (__fcc_ref_dfa) (__fcc_f_solver[1,2]) ///
        (__fcc_f_bmax) (__fcc_f_vmax) (__fcc_f_rssrel) (__fcc_f_resmax) ///
        (__fcc_a_bmax) (__fcc_a_vmax) (__fcc_a_rssrel) (__fcc_a_resmax) ///
        (__fcc_g_bmax) (__fcc_g_resmax) (__fcc_f_reconmax) (__fcc_r_reconmax) ///
        (__fcc_fr_predmax) (__fcc_fg_predmax) ///
        (`science_fail')
    display as result "PREFLIGHT|case=`case_id'|threads=`threads'|fereg_solver=" ///
        __fcc_f_solver[1,2] "|science_fail=`science_fail'|f_b=" __fcc_f_bmax ///
        "|f_v=" __fcc_f_vmax "|f_rss=" __fcc_f_rssrel "|a_b=" __fcc_a_bmax ///
        "|a_v=" __fcc_a_vmax "|a_rss=" __fcc_a_rssrel "|g_b=" __fcc_g_bmax ///
        "|f_recon=" __fcc_f_reconmax "|r_recon=" __fcc_r_reconmax ///
        "|fr_pred=" __fcc_fr_predmax "|fg_pred=" __fcc_fg_predmax ///
        "|dfa_ref=" __fcc_ref_dfa "|dfa_f=" __fcc_f_dfa "|dfa_a=" __fcc_a_dfa
    __fcc_cleanup
    if `science_fail' exit 459
end

capture program drop __fcc_time_arm
program define __fcc_time_arm
    args case_id scale design absorb threads mode phase rep sequence arm
    set processors `threads'
    __fcc_cleanup
    scalar __fcc_n = .
    scalar __fcc_b1 = .
    scalar __fcc_b2 = .
    scalar __fcc_v11 = .
    scalar __fcc_v12 = .
    scalar __fcc_v22 = .
    scalar __fcc_rss = .
    scalar __fcc_r2 = .
    scalar __fcc_dfa = .
    scalar __fcc_requested = .
    scalar __fcc_used = .
    scalar __fcc_iters = .
    scalar __fcc_pilot_iters = .
    scalar __fcc_pilot_contract = .
    scalar __fcc_fallback = .
    scalar __fcc_native = .
    scalar __fcc_precommit = .

    if `"`absorb'"' == "worker" {
        local f_absorb "worker"
        local r_absorb "worker"
        local g_alphas ""
        if `"`mode'"' == "save_fe" {
            local f_absorb "__fcc_f_worker=worker"
            local r_absorb "__fcc_r_worker=worker"
            local g_alphas "alphas(__fcc_g_worker) savecons replace"
        }
    }
    else {
        local f_absorb "worker firm"
        local r_absorb "worker firm"
        local g_alphas ""
        if `"`mode'"' == "save_fe" {
            local f_absorb "__fcc_f_worker=worker __fcc_f_firm=firm"
            local r_absorb "__fcc_r_worker=worker __fcc_r_firm=firm"
            local g_alphas "alphas(__fcc_g_worker __fcc_g_firm) savecons replace"
        }
    }

    timer clear 91
    timer on 91
    if `"`arm'"' == "fereg" {
        capture quietly fereg y x1 x2, absorb(`f_absorb') dof(pairwise) ///
            solver(auto) threads(`threads') tolerance(1e-8) iterate(16000)
    }
    else if `"`arm'"' == "reghdfe" {
        capture quietly reghdfe y x1 x2, absorb(`r_absorb') dof(pairwise) ///
            transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
            tolerance(1e-8) iterate(16000)
    }
    else if `"`arm'"' == "greg" {
        capture quietly greg y x1 x2, absorb(`absorb') algorithm(cg) ///
            hdfetol(1e-8) maxiter(16000) mata(GtoolsRegress) `g_alphas'
    }
    else if `"`arm'"' == "areg" & `"`absorb'"' == "worker" {
        capture quietly areg y x1 x2, absorb(worker)
    }
    else if `"`arm'"' == "areg" {
        capture quietly areg y x1 x2, absorb(`absorb') dfabsorb ///
            noabsorbtest tolerance(1e-8) iterate(16000) nolog
    }
    else {
        display as error "unknown benchmark arm: `arm'"
        exit 198
    }
    local command_rc = _rc
    timer off 91
    quietly timer list 91
    scalar __fcc_seconds = r(t91)
    timer clear 91

    if `command_rc' == 0 & `"`arm'"' == "greg" {
        capture scalar __fcc_n = r(N)
        if _rc local command_rc = 459
        if `command_rc' == 0 {
            mata: st_numscalar("__fcc_b1", GtoolsRegress.b[1,1])
            mata: st_numscalar("__fcc_b2", GtoolsRegress.b[1,2])
            mata: st_numscalar("__fcc_v11", GtoolsRegress.Vcov[1,1])
            mata: st_numscalar("__fcc_v12", GtoolsRegress.Vcov[1,2])
            mata: st_numscalar("__fcc_v22", GtoolsRegress.Vcov[2,2])
        }
    }
    else if `command_rc' == 0 {
        matrix __fcc_b = e(b)
        matrix __fcc_v = e(V)
        scalar __fcc_n = e(N)
        scalar __fcc_b1 = _b[x1]
        scalar __fcc_b2 = _b[x2]
        scalar __fcc_v11 = _se[x1]^2
        scalar __fcc_v22 = _se[x2]^2
        scalar __fcc_v12 = __fcc_v[colnumb(__fcc_v,"x1"),colnumb(__fcc_v,"x2")]
        scalar __fcc_rss = e(rss)
        scalar __fcc_r2 = e(r2)
        capture scalar __fcc_dfa = e(df_a)
        if _rc scalar __fcc_dfa = .
    }
    if `command_rc' == 0 & `"`arm'"' == "fereg" {
        matrix __fcc_solver = e(fereg_solver)
        matrix __fcc_runtime = e(fereg_runtime)
        scalar __fcc_requested = __fcc_solver[1,1]
        scalar __fcc_used = __fcc_solver[1,2]
        scalar __fcc_iters = __fcc_solver[1,3]
        scalar __fcc_fallback = __fcc_solver[1,11]
        scalar __fcc_pilot_iters = __fcc_solver[1,15]
        scalar __fcc_pilot_contract = __fcc_solver[1,16]
        scalar __fcc_native = __fcc_runtime[20,2]
        scalar __fcc_precommit = __fcc_runtime[21,2]
    }

    post __fcc_runpost (`"`case_id'"') (`"`scale'"') (`"`design'"') ///
        (`threads') (`"`mode'"') (`"`phase'"') (`rep') (`sequence') (`"`arm'"') ///
        (`command_rc') (__fcc_seconds) (__fcc_n) (__fcc_b1) (__fcc_b2) ///
        (__fcc_v11) (__fcc_v12) (__fcc_v22) (__fcc_rss) (__fcc_r2) (__fcc_dfa) ///
        (__fcc_requested) (__fcc_used) (__fcc_iters) (__fcc_pilot_iters) ///
        (__fcc_pilot_contract) (__fcc_fallback) (__fcc_native) (__fcc_precommit)
    display as result "RUN|case=`case_id'|threads=`threads'|mode=`mode'|phase=`phase'" ///
        "|rep=`rep'|sequence=`sequence'|arm=`arm'|rc=`command_rc'|seconds=" __fcc_seconds
    if `command_rc' exit `command_rc'
end

postfile __fcc_runpost str40 case_id str12 scale str20 design int threads ///
    str8 mode str5 phase int rep int sequence str10 arm int rc double seconds ///
    double n b1 b2 v11 v12 v22 rss r2 df_a requested_solver used_solver ///
    iterations pilot_iterations pilot_contraction fallback native_seconds ///
    precommit_seconds using `"`output_dir'/command_comparison_runs.dta"', replace
postfile __fcc_prepost str40 case_id str12 scale str20 design int threads ///
    double n df_a fereg_solver fereg_bmax fereg_vmax fereg_rssrel fereg_resmax ///
    areg_bmax areg_vmax areg_rssrel areg_resmax greg_bmax greg_resmax ///
    fereg_reconstruct_max reghdfe_reconstruct_max fereg_reghdfe_predmax ///
    fereg_greg_predmax byte science_fail ///
    using `"`output_dir'/command_comparison_preflights.dta"', replace

local case_index = 0
foreach scale of local scale_spec {
    if `"`scale'"' == "n100k" {
        local n 100000
        local workers 10000
        local firms 500
        local designs "one_fe iid_two_fe akm_highmob akm_standard akm_lowmob"
    }
    else if `"`scale'"' == "n1m" {
        local n 1000000
        local workers 100000
        local firms 5000
        local designs "one_fe iid_two_fe akm_standard akm_lowmob"
    }
    else if `"`scale'"' == "n10m" {
        local n 10000000
        local workers 1000000
        local firms 50000
        local designs "one_fe iid_two_fe akm_standard akm_lowmob"
    }
    else {
        display as error "unknown scale: `scale'"
        exit 198
    }
    if `"`design_spec'"' != "all" {
        if !strpos(" `designs' ", " `design_spec' ") {
            display as error "design `design_spec' is not registered for scale `scale'"
            exit 198
        }
        local designs "`design_spec'"
    }

    foreach design of local designs {
        local ++case_index
        if `"`design'"' == "one_fe" {
            local mobility .15
            local absorb "worker"
            local seed 2026082701
        }
        else if `"`design'"' == "iid_two_fe" {
            local mobility 1
            local absorb "worker firm"
            local seed 2026082702
        }
        else if `"`design'"' == "akm_highmob" {
            local mobility .75
            local absorb "worker firm"
            local seed 2026082703
        }
        else if `"`design'"' == "akm_standard" {
            local mobility .15
            local absorb "worker firm"
            local seed 2026082704
        }
        else if `"`design'"' == "akm_lowmob" {
            local mobility .02
            local absorb "worker firm"
            local seed 2026082705
        }
        local case_id "`design'_`scale'"
        if `"`data_path'"' == "" {
            display as text "GENERATE|case=`case_id'|n=`n'|workers=`workers'|firms=`firms'"
            __fcc_generate `n' `workers' `firms' 10 `mobility' `seed'
        }
        else {
            capture confirm file `"`data_path'"'
            if _rc {
                display as text "GENERATE_SHARED|case=`case_id'|path=`data_path'"
                __fcc_generate `n' `workers' `firms' 10 `mobility' `seed'
                quietly save `"`data_path'"', replace
            }
            else {
                display as text "LOAD_SHARED|case=`case_id'|path=`data_path'"
                quietly use `"`data_path'"', clear
                assert _N == `n'
                confirm variable worker firm period x1 x2 y
            }
        }

        local thread_index = 0
        foreach threads of local thread_spec {
            local ++thread_index
            local areg_enabled = (`"`areg_scope'"' == "all") | ///
                (`"`areg_scope'"' == "one_fe" & `"`design'"' == "one_fe")
            if `"`preflight_spec'"' == "on" {
                __fcc_preflight `case_id' `scale' `design' `"`absorb'"' ///
                    `threads' `areg_enabled'
            }

            local modes "fit save_fe"
            if inlist(`"`mode_spec'"', "fit", "save_fe") local modes "`mode_spec'"
            if `"`mode_spec'"' == "none" local modes ""
            foreach mode of local modes {
                if `"`mode'"' == "fit" {
                    local arms "fereg greg reghdfe"
                    if `areg_enabled' local arms "`arms' areg"
                }
                else local arms "fereg greg reghdfe"
                if `"`arm_spec'"' != "all" {
                    if `"`arm_spec'"' == "none" local arms ""
                    else if strpos(" `arms' ", " `arm_spec' ") local arms "`arm_spec'"
                    else local arms ""
                }
                local arm_count : word count `arms'

                if `arm_count' > 0 & `"`cold_spec'"' == "on" {
                    local shift = mod(`case_index' + `thread_index' - 2, `arm_count')
                    forvalues sequence = 1/`arm_count' {
                        local arm_index = mod(`sequence' + `shift' - 2, `arm_count') + 1
                        local arm : word `arm_index' of `arms'
                        __fcc_time_arm `case_id' `scale' `design' `"`absorb'"' ///
                            `threads' `mode' cold 0 `sequence' `arm'
                    }
                }
                if `arm_count' > 0 & `warm_reps' > 0 {
                    forvalues rep = 1/`warm_reps' {
                        local global_rep = `rep' + `rep_offset'
                        local shift = mod(`case_index' + `thread_index' + `global_rep' - 3, `arm_count')
                        forvalues sequence = 1/`arm_count' {
                            local arm_index = mod(`sequence' + `shift' - 2, `arm_count') + 1
                            local arm : word `arm_index' of `arms'
                            __fcc_time_arm `case_id' `scale' `design' `"`absorb'"' ///
                                `threads' `mode' warm `global_rep' `sequence' `arm'
                        }
                    }
                }
            }
        }
    }
}

postclose __fcc_runpost
postclose __fcc_prepost
use `"`output_dir'/command_comparison_runs.dta"', clear
sort scale design threads mode phase rep arm
export delimited using `"`output_dir'/command_comparison_runs.csv"', replace
use `"`output_dir'/command_comparison_preflights.dta"', clear
sort scale design threads
export delimited using `"`output_dir'/command_comparison_preflights.csv"', replace

tempname success
file open `success' using `"`output_dir'/SUCCESS.txt"', write text replace
file write `success' "status=success" _n
file write `success' "completed_date=`c(current_date)'" _n
file write `success' "completed_time=`c(current_time)'" _n
file close `success'
display as result "END_COMMAND_COMPARISON|rc=0"
exit 0
