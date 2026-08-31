/* Scientific parity gate for one N/thread cell; never used as a timing. */
version 19.0
clear all
set more off
set linesize 255

args source_root output_dir comparator_root data_path n requested_threads
if `"`source_root'"' == "" | `"`output_dir'"' == "" | ///
        `"`comparator_root'"' == "" | `"`data_path'"' == "" {
    display as error "source_root, output_dir, comparator_root, and data_path are required"
    exit 198
}
local stata_processors = min(`requested_threads', c(processors_lic), c(processors_mach))
set processors `stata_processors'
sysdir set PLUS `"`comparator_root'"'
adopath ++ `"`source_root'/stata"'
adopath ++ `"`source_root'/dist"'
foreach command in fereg reghdfe areg {
    capture findfile `command'.ado
    if _rc {
        display as error "required command unavailable: `command'"
        exit _rc
    }
}
quietly use `"`data_path'"', clear
assert _N == `n'
local xvars "x1 x2 x3 x4 x5 x6 x7 x8 x9 x10"

quietly reghdfe y `xvars', absorb(worker firm) dof(pairwise) ///
    transform(symmetric_kaczmarz) acceleration(conjugate_gradient) ///
    tolerance(1e-10) iterate(16000) residuals(__ref_res)
matrix __ref_b = e(b)
matrix __ref_v = e(V)
scalar __ref_n = e(N)
scalar __ref_rss = e(rss)
scalar __ref_dfa = e(df_a)

quietly fereg y `xvars', absorb(worker firm) dof(pairwise) solver(auto) ///
    threads(`requested_threads') tolerance(1e-10) iterate(16000) ///
    residuals(__f_res)
matrix __f_b = e(b)
matrix __f_v = e(V)
scalar __f_n = e(N)
scalar __f_rss = e(rss)
scalar __f_dfa = e(df_a)
scalar __f_threads = e(fereg_threads)
scalar __f_requested = e(fereg_requested_threads)
mata: st_numscalar("__bscale", max((max(abs(st_matrix("__ref_b"))), 1)))
mata: st_numscalar("__vscale", max((max(abs(st_matrix("__ref_v"))), 1)))
mata: st_numscalar("__f_bmax", max(abs(st_matrix("__ref_b") :- st_matrix("__f_b"))))
mata: st_numscalar("__f_vmax", max(abs(st_matrix("__ref_v") :- st_matrix("__f_v"))))
scalar __f_rssrel = abs(__f_rss - __ref_rss) / max(abs(__ref_rss), 1)
generate double __diff = abs(__f_res - __ref_res)
quietly summarize __diff, meanonly
scalar __f_resmax = r(max)
drop __diff

quietly areg y `xvars', absorb(worker firm) dfabsorb noabsorbtest ///
    tolerance(1e-10) iterate(16000) nolog
matrix __a_b = e(b)
matrix __a_v = e(V)
scalar __a_n = e(N)
scalar __a_rss = e(rss)
scalar __a_dfa = e(df_a)
predict double __a_res, residuals
mata: st_numscalar("__a_bmax", max(abs(st_matrix("__ref_b") :- st_matrix("__a_b"))))
mata: st_numscalar("__a_vmax", max(abs(st_matrix("__ref_v") :- st_matrix("__a_v"))))
scalar __a_rssrel = abs(__a_rss - __ref_rss) / max(abs(__ref_rss), 1)
generate double __diff = abs(__a_res - __ref_res)
quietly summarize __diff, meanonly
scalar __a_resmax = r(max)
drop __diff

scalar __science_fail = 0
if __ref_n != `n' | __f_n != `n' | __a_n != `n' scalar __science_fail = 1
if __f_requested != `requested_threads' | __f_threads != `requested_threads' scalar __science_fail = 1
if __f_bmax > 1e-10 + 1e-7*__bscale scalar __science_fail = 1
if __a_bmax > 1e-10 + 1e-7*__bscale scalar __science_fail = 1
if __f_vmax > 1e-10 + 1e-6*__vscale scalar __science_fail = 1
if __a_vmax > 1e-10 + 1e-6*__vscale scalar __science_fail = 1
if __f_rssrel > 1e-7 | __a_rssrel > 1e-7 scalar __science_fail = 1

clear
quietly set obs 1
generate long n = `n'
generate int requested_threads = `requested_threads'
generate int stata_processors = `stata_processors'
generate double reference_n = __ref_n
generate double reference_df_a = __ref_dfa
generate double fereg_df_a = __f_dfa
generate double areg_df_a = __a_dfa
generate double fereg_bmax = __f_bmax
generate double fereg_vmax = __f_vmax
generate double fereg_rssrel = __f_rssrel
generate double fereg_resmax = __f_resmax
generate double areg_bmax = __a_bmax
generate double areg_vmax = __a_vmax
generate double areg_rssrel = __a_rssrel
generate double areg_resmax = __a_resmax
generate int fereg_requested_threads = __f_requested
generate int fereg_effective_threads = __f_threads
generate byte science_fail = __science_fail
export delimited using `"`output_dir'/science.csv"', replace

tempname success
file open `success' using `"`output_dir'/SUCCESS.txt"', write text replace
file write `success' "status=success" _n
file close `success'
display as result "PAPER_AKM_PREFLIGHT|n=`n'|requested=`requested_threads'|stata=`stata_processors'|science_fail=" __science_fail
if __science_fail exit 459
exit 0
