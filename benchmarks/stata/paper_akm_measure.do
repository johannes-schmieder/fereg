/* One fresh-process timing. Dataset loading and baseline stabilization are untimed. */
version 19.0
clear all
set more off
set linesize 255

args source_root output_dir comparator_root data_path arm n requested_threads rep
if `"`source_root'"' == "" | `"`output_dir'"' == "" | ///
        `"`comparator_root'"' == "" | `"`data_path'"' == "" | `"`arm'"' == "" {
    display as error "all measurement arguments are required"
    exit 198
}
if !inlist(`"`arm'"', "fereg", "reghdfe", "areg") exit 198
local stata_processors = min(`requested_threads', c(processors_lic), c(processors_mach))
set processors `stata_processors'
sysdir set PLUS `"`comparator_root'"'
adopath ++ `"`source_root'/stata"'
adopath ++ `"`source_root'/dist"'
capture findfile `arm'.ado
if _rc exit _rc
quietly use `"`data_path'"', clear
assert _N == `n'
local xvars "x1 x2 x3 x4 x5 x6 x7 x8 x9 x10"

tempname ready
file open `ready' using `"`output_dir'/READY"', write text replace
file write `ready' "ready" _n
file close `ready'
sleep 1000
tempname start
file open `start' using `"`output_dir'/START"', write text replace
file write `start' "start" _n
file close `start'

timer clear 91
timer on 91
if `"`arm'"' == "fereg" {
    capture quietly fereg y `xvars', absorb(worker firm) dof(pairwise) ///
        solver(auto) threads(`requested_threads') tolerance(1e-8) iterate(16000)
}
else if `"`arm'"' == "reghdfe" {
    capture quietly reghdfe y `xvars', absorb(worker firm) dof(pairwise) ///
        transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
        tolerance(1e-8) iterate(16000)
}
else {
    capture quietly areg y `xvars', absorb(worker firm) dfabsorb ///
        noabsorbtest tolerance(1e-8) iterate(16000) nolog
}
local command_rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91

tempname done
file open `done' using `"`output_dir'/DONE"', write text replace
file write `done' "done" _n
file close `done'

scalar __en = .
scalar __rss = .
scalar __dfa = .
scalar __effective = .
scalar __requested = .
if `command_rc' == 0 {
    scalar __en = e(N)
    scalar __rss = e(rss)
    scalar __dfa = e(df_a)
}
if `command_rc' == 0 & `"`arm'"' == "fereg" {
    scalar __effective = e(fereg_threads)
    scalar __requested = e(fereg_requested_threads)
}

clear
quietly set obs 1
generate str10 arm = `"`arm'"'
generate long n = `n'
generate int requested_threads = `requested_threads'
generate int stata_processors = `stata_processors'
generate int rep = `rep'
generate int rc = `command_rc'
generate double seconds = __seconds
generate double e_n = __en
generate double rss = __rss
generate double df_a = __dfa
generate double fereg_requested_threads = __requested
generate double fereg_effective_threads = __effective
generate str8 stata_version = string(c(stata_version))
generate str12 stata_flavor = c(flavor)
generate int processors_lic = c(processors_lic)
generate int processors_mach = c(processors_mach)
export delimited using `"`output_dir'/result.csv"', replace
display as result "PAPER_AKM_MEASURE|arm=`arm'|n=`n'|requested=`requested_threads'|stata=`stata_processors'|rep=`rep'|rc=`command_rc'|seconds=" __seconds
if `command_rc' exit `command_rc'
exit 0
