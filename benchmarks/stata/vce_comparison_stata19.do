/* Exact-scope OLS VCE comparison for Linux/SCC qualification. */
version 19.0
clear all
set more off
set linesize 255

args repo_root output_dir comparator_root data_path controls vce requested_threads warm_reps weight_kind
if `"`repo_root'"' == "" | `"`output_dir'"' == "" | `"`data_path'"' == "" {
    display as error "repo_root, output_dir, and data_path are required"
    exit 198
}
if `"`comparator_root'"' == "" local comparator_root "system"
if `"`controls'"' == "" local controls 10
if `"`vce'"' == "" local vce "robust"
if `"`requested_threads'"' == "" local requested_threads 1
if `"`warm_reps'"' == "" local warm_reps 5
if `"`weight_kind'"' == "" local weight_kind "none"
if !inlist(`controls', 0, 10, 30) exit 198
if !inlist(`"`vce'"', "iid", "robust", "cluster1", "cluster2", "cluster3", "cluster4", "cluster5") exit 198
if !inlist(`requested_threads', 1, 16) exit 198
if !inlist(`"`weight_kind'"', "none", "aweight", "pweight") exit 198
if `"`weight_kind'"' != "none" & !inlist(`"`vce'"', "robust", "cluster1") exit 198

capture mkdir `"`output_dir'"'
if !inlist(`"`comparator_root'"', "", "system") sysdir set PLUS `"`comparator_root'"'
adopath ++ `"`repo_root'/stata"'
adopath ++ `"`repo_root'/dist"'
global GTOOLS_BETA "I KNOW WHAT I AM DOING"
local stata_processors = min(`requested_threads', c(processors_lic), c(processors_mach))
set processors `stata_processors'

foreach command in fereg reghdfe greg gregress {
    capture findfile `command'.ado
    if _rc exit _rc
}

use `"`data_path'"', clear
generate long __frg_cluster3 = mod(17*worker + year, 997) + 1
generate long __frg_cluster4 = mod(31*firm + worker, 499) + 1
generate long __frg_cluster5 = mod(47*worker + 13*firm + year, 251) + 1
generate double __frg_aw = 0.5 + mod(worker, 17)/10
generate double __frg_pw = 0.5 + mod(firm, 13)/10

local xvars ""
local greg_xvars ""
if `controls' > 0 {
    forvalues j = 1/`controls' {
        local xvars `"`xvars' c.x`j'"'
        local greg_xvars `"`greg_xvars' x`j'"'
    }
}
local weightopt ""
if `"`weight_kind'"' == "aweight" local weightopt "[aw=__frg_aw]"
if `"`weight_kind'"' == "pweight" local weightopt "[pw=__frg_pw]"
local f_vce ""
local r_vce ""
local g_vce ""
if `"`vce'"' == "robust" {
    local f_vce "vce(robust)"
    local r_vce "vce(robust)"
    local g_vce "robust"
}
if `"`vce'"' == "cluster1" {
    local f_vce "vce(cluster firm)"
    local r_vce "vce(cluster firm)"
    local g_vce "cluster(firm)"
}
if `"`vce'"' == "cluster2" {
    local f_vce "vce(cluster firm year)"
    local r_vce "vce(cluster firm year)"
}
if `"`vce'"' == "cluster3" {
    local f_vce "vce(cluster firm year __frg_cluster3)"
    local r_vce "vce(cluster firm year __frg_cluster3)"
}
if `"`vce'"' == "cluster4" {
    local f_vce "vce(cluster firm year __frg_cluster3 __frg_cluster4)"
    local r_vce "vce(cluster firm year __frg_cluster3 __frg_cluster4)"
}
if `"`vce'"' == "cluster5" {
    local f_vce "vce(cluster firm year __frg_cluster3 __frg_cluster4 __frg_cluster5)"
    local r_vce "vce(cluster firm year __frg_cluster3 __frg_cluster4 __frg_cluster5)"
}
local include_greg = inlist(`"`vce'"', "iid", "robust", "cluster1")

quietly reghdfe y `xvars' `weightopt', absorb(worker firm) dof(pairwise) `r_vce' ///
    transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
    tolerance(1e-10) iterate(16000)
matrix __frg_ref_b = e(b)
matrix __frg_ref_v = e(V)
scalar __frg_ref_n = e(N)

quietly fereg y `xvars' `weightopt', absorb(worker firm) dof(pairwise) `f_vce' ///
    solver(auto) threads(`requested_threads') tolerance(1e-10) iterate(16000)
matrix __frg_f_b = e(b)
matrix __frg_f_v = e(V)
scalar __frg_f_n = e(N)
scalar __frg_f_threads = e(fereg_threads)
mata: st_numscalar("__frg_bmax", max(abs(st_matrix("__frg_ref_b") :- st_matrix("__frg_f_b"))))
mata: st_numscalar("__frg_vmax", max(abs(st_matrix("__frg_ref_v") :- st_matrix("__frg_f_v"))))
mata: st_numscalar("__frg_bscale", max((1, max(abs(st_matrix("__frg_ref_b"))))))

scalar __frg_g_n = __frg_ref_n
scalar __frg_g_bmax = 0
scalar __frg_g_semax = 0
if `include_greg' {
    quietly greg y `greg_xvars' `weightopt', absorb(worker firm) `g_vce' ///
        algorithm(cg) hdfetol(1e-10) maxiter(16000) mata(GtoolsRegress)
    scalar __frg_g_n = r(N)
    if `controls' > 0 {
        mata: __frg_ref = st_matrix("__frg_ref_b")
        mata: __frg_g_bmax_m = 0
        mata: __frg_g_semax_m = 0
        forvalues j = 1/`controls' {
            mata: __frg_g_bmax_m = max((__frg_g_bmax_m, abs(__frg_ref[1, `j'] - GtoolsRegress.b[1, `j'])))
            mata: __frg_g_semax_m = max((__frg_g_semax_m, abs(sqrt(st_matrix("__frg_ref_v")[`j', `j']) - GtoolsRegress.se[1, `j'])))
        }
        mata: st_numscalar("__frg_g_bmax", __frg_g_bmax_m)
        mata: st_numscalar("__frg_g_semax", __frg_g_semax_m)
        mata: mata drop __frg_ref __frg_g_bmax_m __frg_g_semax_m
    }
}

local science_fail = 0
if __frg_f_n != __frg_ref_n | __frg_f_threads != `requested_threads' local science_fail = 1
if __frg_bmax > 1e-10 + 1e-7*__frg_bscale local science_fail = 1
if inlist(`"`vce'"', "iid", "robust", "cluster1") & __frg_vmax > 2e-6 local science_fail = 1
if `include_greg' & (__frg_g_n != __frg_ref_n | __frg_g_bmax > 1e-7) local science_fail = 1

tempname preflight
file open `preflight' using `"`output_dir'/preflight.csv"', write text replace
file write `preflight' "controls,vce,threads,weight,N,bmax_reghdfe,vmax_reghdfe,bmax_greg,semax_greg,science_fail" _n
file write `preflight' `"`controls',`vce',`requested_threads',`weight_kind',"' ///
    %21.12g (__frg_ref_n) "," %21.12g (__frg_bmax) "," %21.12g (__frg_vmax) "," ///
    %21.12g (__frg_g_bmax) "," %21.12g (__frg_g_semax) ",`science_fail'" _n
file close `preflight'
if `science_fail' exit 459

foreach arm in fereg reghdfe {
    if `"`arm'"' == "fereg" quietly fereg y `xvars' `weightopt', absorb(worker firm) dof(pairwise) `f_vce' solver(auto) threads(`requested_threads') tolerance(1e-8) iterate(16000)
    if `"`arm'"' == "reghdfe" quietly reghdfe y `xvars' `weightopt', absorb(worker firm) dof(pairwise) `r_vce' transform(symmetric_kaczmarz) acceleration(conjugate_gradient) tolerance(1e-8) iterate(16000)
}
if `include_greg' quietly greg y `greg_xvars' `weightopt', absorb(worker firm) `g_vce' algorithm(cg) hdfetol(1e-8) maxiter(16000) mata(GtoolsRegress)

tempname runs
file open `runs' using `"`output_dir'/runs.csv"', write text replace
file write `runs' "arm,rep,seconds,native_seconds,N,command_rc" _n
forvalues rep = 1/`warm_reps' {
    local order "fereg reghdfe"
    if `include_greg' local order "fereg greg reghdfe"
    if mod(`rep', 2) == 0 {
        local order "reghdfe fereg"
        if `include_greg' local order "reghdfe greg fereg"
    }
    foreach arm of local order {
        timer clear 91
        timer on 91
        if `"`arm'"' == "fereg" capture quietly fereg y `xvars' `weightopt', absorb(worker firm) dof(pairwise) `f_vce' solver(auto) threads(`requested_threads') tolerance(1e-8) iterate(16000)
        if `"`arm'"' == "reghdfe" capture quietly reghdfe y `xvars' `weightopt', absorb(worker firm) dof(pairwise) `r_vce' transform(symmetric_kaczmarz) acceleration(conjugate_gradient) tolerance(1e-8) iterate(16000)
        if `"`arm'"' == "greg" capture quietly greg y `greg_xvars' `weightopt', absorb(worker firm) `g_vce' algorithm(cg) hdfetol(1e-8) maxiter(16000) mata(GtoolsRegress)
        local command_rc = _rc
        timer off 91
        quietly timer list 91
        scalar __frg_seconds = r(t91)
        scalar __frg_native = .
        scalar __frg_n = .
        if `command_rc' == 0 & `"`arm'"' == "fereg" {
            scalar __frg_native = el(e(fereg_runtime), 20, 2)
            scalar __frg_n = e(N)
        }
        if `command_rc' == 0 & `"`arm'"' == "reghdfe" scalar __frg_n = e(N)
        if `command_rc' == 0 & `"`arm'"' == "greg" scalar __frg_n = r(N)
        file write `runs' `"`arm',`rep',"' %21.12g (__frg_seconds) "," ///
            %21.12g (__frg_native) "," %21.12g (__frg_n) ",`command_rc'" _n
        if `command_rc' exit `command_rc'
    }
}
file close `runs'

tempname environment
file open `environment' using `"`output_dir'/environment.txt"', write text replace
file write `environment' "stata_version=`c(stata_version)'" _n
file write `environment' "stata_flavor=`c(flavor)'" _n
file write `environment' "processors_lic=`c(processors_lic)'" _n
file write `environment' "processors_mach=`c(processors_mach)'" _n
file write `environment' "stata_processors=`stata_processors'" _n
file write `environment' "requested_threads=`requested_threads'" _n
file write `environment' `"controls=`controls'"' _n
file write `environment' `"vce=`vce'"' _n
file write `environment' `"weight_kind=`weight_kind'"' _n
file close `environment'
display as result "FEREG_VCE_COMPARISON_SUCCESS"
exit 0
