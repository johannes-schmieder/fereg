/*
    Prepare identical Veneto samples and benchmark fereg for comparison with
    tests/manual/simple_Veneto_fixest.R.

    Required globals:

        FEREG_VENETO_DATA         pinned LeaveOutKSS test.csv
        FEREG_FIXEST_OUTPUT_DIR   directory for samples and timing CSVs

    The companion R script excludes R startup and CSV loading from its timed
    region.  This diagnostic uses native fereg defaults and is not release
    evidence.
*/

version 19.0
clear all
set more off
set varabbrev off
set linesize 255

local repetitions 10
local requested_threads 8
local data_file `"$FEREG_VENETO_DATA"'
local output_dir `"$FEREG_FIXEST_OUTPUT_DIR"'

if strtrim(`"`data_file'"') == "" {
    display as error "set FEREG_VENETO_DATA to the pinned Veneto CSV"
    exit 198
}
if strtrim(`"`output_dir'"') == "" {
    display as error "set FEREG_FIXEST_OUTPUT_DIR to an existing directory"
    exit 198
}
confirm file `"`data_file'"'

local processor_cap = min(c(processors_lic), c(processors_mach))
local threads = min(`requested_threads', `processor_cap')
quietly set processors `threads'

adopath + "./stata"
adopath + "./dist"
capture which fereg
if _rc {
    display as error "fereg is not installed or not on the current adopath"
    exit 111
}

capture mata: mata drop __fvenfx_mark_largest_component()
mata:
void __fvenfx_mark_largest_component(string scalar worker_name,
                                      string scalar firm_name,
                                      string scalar marker_name)
{
    real colvector worker, firm, parent, tree_size, roots, marker
    real scalar workers, firms, vertices, observation, left, right
    real scalar temporary, largest_vertices, largest_root

    worker = st_data(., worker_name)
    firm = st_data(., firm_name)
    workers = max(worker)
    firms = max(firm)
    vertices = workers + firms
    parent = 1::vertices
    tree_size = J(vertices, 1, 1)

    for (observation = 1; observation <= rows(worker); observation++) {
        left = worker[observation]
        right = workers + firm[observation]
        while (parent[left] != left) left = parent[left]
        while (parent[right] != right) right = parent[right]
        if (left != right) {
            if (tree_size[left] < tree_size[right]) {
                temporary = left
                left = right
                right = temporary
            }
            parent[right] = left
            tree_size[left] = tree_size[left] + tree_size[right]
        }
    }

    roots = selectindex(parent :== (1::vertices))
    largest_vertices = max(tree_size[roots])
    largest_root = roots[selectindex(tree_size[roots] :== largest_vertices)[1]]
    marker = J(rows(worker), 1, 0)
    for (observation = 1; observation <= rows(worker); observation++) {
        left = worker[observation]
        while (parent[left] != left) left = parent[left]
        marker[observation] = left == largest_root
    }
    st_store(., marker_name, marker)
}
end

import delimited using `"`data_file'"', varnames(nonames) asdouble clear
assert c(k) == 10
rename v1 worker
rename v2 firm
rename v3 year
rename v4 outcome
keep worker firm year outcome
confirm numeric variable worker firm year outcome
assert _N == 71614
isid worker year
sort worker year

/* Freeze fereg's post-singleton sample before either implementation is timed. */
quietly fereg outcome, absorb(worker firm) dof(pairwise) threads(`threads')
generate byte __fvenfx_common_sample = e(sample)
keep if __fvenfx_common_sample
drop __fvenfx_common_sample
local full_n = _N
assert `full_n' == 69970

tempfile full_sample largest_sample
save `full_sample', replace
export delimited using `"`output_dir'/veneto_post_singleton.csv"', replace

tempvar worker_code firm_code largest_connected
egen long `worker_code' = group(worker)
egen long `firm_code' = group(firm)
generate byte `largest_connected' = 0
mata: __fvenfx_mark_largest_component("`worker_code'", "`firm_code'", "`largest_connected'")
keep if `largest_connected'
drop `worker_code' `firm_code' `largest_connected'
local largest_n = _N
assert `largest_n' == 62178
save `largest_sample', replace
export delimited using `"`output_dir'/veneto_largest_connected.csv"', replace

tempname run_post
tempfile fereg_runs
postfile `run_post' str24 sample str16 estimator int repetition       ///
    double elapsed_seconds observations rss solver_code              ///
    primary_iterations residual_iterations fallback                  ///
    using `fereg_runs', replace

foreach sample in post_singleton largest_connected {
    if "`sample'" == "post_singleton" {
        use `full_sample', clear
        local expected_n = `full_n'
    }
    else {
        use `largest_sample', clear
        local expected_n = `largest_n'
    }

    local command_fereg_auto                                         ///
        "fereg outcome, absorb(worker firm) dof(pairwise) threads(`threads')"
    local command_fereg_cmg                                          ///
        "fereg outcome, absorb(worker firm) dof(pairwise) threads(`threads') solver(CMG)"

    foreach arm in fereg_auto fereg_cmg {
        capture quietly `command_`arm''
        if _rc {
            local warm_rc = _rc
            display as error "`sample' `arm' warm-up failed with r(`warm_rc')"
            exit `warm_rc'
        }
        assert e(N) == `expected_n'
    }

    forvalues repetition = 1/`repetitions' {
        if mod(`repetition', 2) == 1 local command_order             ///
            "fereg_auto fereg_cmg"
        else local command_order "fereg_cmg fereg_auto"

        foreach arm of local command_order {
            timer clear 91
            timer on 91
            capture quietly `command_`arm''
            local run_rc = _rc
            timer off 91
            quietly timer list 91
            local elapsed_seconds = r(t91)
            if `run_rc' {
                display as error                                   ///
                    "`sample' `arm' failed in repetition `repetition' with r(`run_rc')"
                exit `run_rc'
            }
            assert e(N) == `expected_n'
            local solver_code = e(fereg_solver)[1,2]
            local primary_iterations = e(fereg_solver)[1,3]
            local residual_iterations = e(fereg_solver)[2,3]
            local fallback = e(fereg_solver)[1,11]
            post `run_post' ("`sample'") ("`arm'") (`repetition')   ///
                (`elapsed_seconds') (e(N)) (e(rss)) (`solver_code')  ///
                (`primary_iterations') (`residual_iterations')      ///
                (`fallback')
        }
    }
}
postclose `run_post'

use `fereg_runs', clear
sort sample estimator repetition
format elapsed_seconds rss %21.17g
export delimited using `"`output_dir'/fereg_runs.csv"', replace datafmt

preserve
collapse (mean) mean_seconds=elapsed_seconds                         ///
    (median) median_seconds=elapsed_seconds                          ///
    (min) min_seconds=elapsed_seconds                                ///
    (max) max_seconds=elapsed_seconds                                ///
    (firstnm) observations rss solver_code primary_iterations       ///
        residual_iterations fallback, by(sample estimator)
sort sample estimator
format mean_seconds median_seconds min_seconds max_seconds rss %21.17g
export delimited using `"`output_dir'/fereg_summary.csv"', replace datafmt
list, noobs abbreviate(24)
restore

display as result                                                   ///
    "FEREG_VENETO_FIXEST_PREP|PASS|full_N=`full_n'|largest_N=`largest_n'|threads=`threads'|repetitions=`repetitions'"
