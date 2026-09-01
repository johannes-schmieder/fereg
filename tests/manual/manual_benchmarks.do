/*
    Referee-facing performance benchmarks for fereg.

    This do-file generates synthetic panel data and compares elapsed time for
    fereg, reghdfe, Stata 19 areg, and Greg.  Data generation, command loading,
    and one warm-up call per command are outside the recorded timings.  Timed
    command order rotates across repetitions.  The resulting run-level data,
    a median summary, and two timing figures are written to output_directory.

    The benchmark measures two slices rather than the full size-by-thread
    Cartesian product.  The first holds requested threads at eight and varies
    dataset size.  The second holds the dataset at its medium level and varies
    requested cores from one through sixteen.  Their shared medium/eight-core
    cell is run once.  Both figures place the two-FE dense, two-FE
    sparse, and three-FE sparse designs in three side-by-side panels.

    Dense and sparse AKM designs use 20% and 2% worker mobility, respectively.
    In three-FE designs, observation-level counties cross workers and firms so
    the third absorbed effect is not nested in either of the first two.

    Stata does not expose a comparable per-command peak-memory measurement.
    fereg's own process-RSS and native-allocation diagnostics are retained, but
    they are unavailable for the other commands and are not plotted as if they
    were comparable.  Comparable peak RSS requires an external fresh-process
    sampler, as used by the repository's release benchmark harness.

    This is an accessible diagnostic benchmark, not release evidence.  For
    correctness checks, run tests/manual/manual_validation.do separately.
*/

version 19.0
clear all
set more off
set linesize 255
capture graph drop _all

/* ---------------------------- User settings ---------------------------- */
local scale               .1                 // Multiplies every base dataset size.
local problem_types       "two_fe_dense two_fe_sparse three_fe_sparse" // Three figure panels.
local dataset_sizes       "25000 50000 100000 150000 250000" // Size-figure levels before scaling.
local medium_dataset_size 100000             // Base size used in the thread figure.
local fixed_threads       8                  // Requested threads used in the size figure.
local thread_counts       "1 2 4 8 16"      // Requested threads used at the medium size.
local controls            4                  // Number of continuous controls in each regression.
local repetitions         5                  // Recorded warm repetitions per command and benchmark cell.
local seed                20260831           // Random-number seed for reproducible datasets.
local tuning_settings     "default"          // "default" or "harmonized".
local common_tolerance    1e-8               // Used only with "harmonized".
local common_maxiter      16000              // Used only with "harmonized".
local output_directory    "."                // Directory for datasets and figures.
local make_figures        1                  // Export timing figures (0: retain datasets only).
local require_comparators 0                  // Fail if reghdfe or Greg is missing (0: record skips).
local strict              1                  // Return r(459) after outputs if any attempted command fails.
/* ----------------------------------------------------------------------- */

/* Accepted problem types:
       one_fe
       two_fe_dense
       two_fe_sparse
       three_fe_dense
       three_fe_sparse
*/

if c(stata_version) < 19 {
    display as error "manual_benchmarks.do requires Stata 19 or newer"
    exit 9
}
if missing(`scale') | `scale' <= 0 {
    display as error "scale must be a positive number"
    exit 198
}
if `controls' < 1 | `controls' > 20 | `controls' != floor(`controls') {
    display as error "controls must be an integer between 1 and 20"
    exit 198
}
if `repetitions' < 1 | `repetitions' != floor(`repetitions') {
    display as error "repetitions must be a positive integer"
    exit 198
}
local tuning_settings = lower(strtrim(`"`tuning_settings'"'))
if !inlist(`"`tuning_settings'"', "default", "harmonized") {
    display as error "tuning_settings must be default or harmonized"
    exit 198
}
local harmonize_settings = (`"`tuning_settings'"' == "harmonized")
if !inlist(`make_figures', 0, 1) | !inlist(`require_comparators', 0, 1) | ///
   !inlist(`strict', 0, 1) {
    display as error ///
        "make_figures, require_comparators, and strict must be 0 or 1"
    exit 198
}
if `harmonize_settings' &                                          ///
        (missing(`common_tolerance') | `common_tolerance' <= 0 |   ///
         missing(`common_maxiter') | `common_maxiter' < 1 |        ///
         `common_maxiter' != floor(`common_maxiter')) {
    display as error ///
        "common_tolerance must be positive and common_maxiter a positive integer"
    exit 198
}

local allowed_problem_types                                      ///
    "one_fe two_fe_dense two_fe_sparse three_fe_dense three_fe_sparse"
if strtrim(`"`problem_types'"') == "all" {
    local selected_problem_types `"`allowed_problem_types'"'
}
else {
    local selected_problem_types
    foreach problem_type of local problem_types {
        local valid_problem : list problem_type in allowed_problem_types
        if !`valid_problem' {
            display as error "unknown problem type: `problem_type'"
            exit 198
        }
        local selected_problem_types `"`selected_problem_types' `problem_type'"'
    }
    local selected_problem_types = strtrim(`"`selected_problem_types'"')
}

local medium_found 0
foreach base_n of local dataset_sizes {
    if missing(real(`"`base_n'"')) | real(`"`base_n'"') <= 0 |       ///
       real(`"`base_n'"') != floor(real(`"`base_n'"')) {
        display as error "dataset_sizes must contain positive integers"
        exit 198
    }
    if real(`"`base_n'"') == `medium_dataset_size' local medium_found 1
}
if !`medium_found' {
    display as error "medium_dataset_size must be one of dataset_sizes"
    exit 198
}
local fixed_found 0
foreach requested_threads of local thread_counts {
    if missing(real(`"`requested_threads'"')) |                       ///
       real(`"`requested_threads'"') < 1 |                            ///
       real(`"`requested_threads'"') > 64 |                           ///
       real(`"`requested_threads'"') != floor(real(`"`requested_threads'"')) {
        display as error "thread_counts must contain integers from 1 through 64"
        exit 198
    }
    if real(`"`requested_threads'"') == `fixed_threads' local fixed_found 1
}
if !`fixed_found' | missing(`fixed_threads') | `fixed_threads' < 1 | ///
        `fixed_threads' > 64 | `fixed_threads' != floor(`fixed_threads') {
    display as error "fixed_threads must be an integer in thread_counts"
    exit 198
}

capture mkdir `"`output_directory'"'
local output_stem `"`output_directory'/manual_benchmarks"'
local original_processors = c(processors)
local original_gtools_beta `"$GTOOLS_BETA"'

capture which fereg
if _rc {
    display as error "fereg is not installed or not on the current adopath"
    exit 111
}
local have_fereg 1
capture noisily which fereg

capture which reghdfe
local have_reghdfe = (_rc == 0)
if `have_reghdfe' capture noisily which reghdfe
else display as text "reghdfe not found; its rows will be marked skip"

capture which greg
local have_greg = (_rc == 0)
if `have_greg' {
    capture which gregress
    local have_greg = (_rc == 0)
}
if `have_greg' {
    capture noisily which greg
    capture noisily which gregress
}
else display as text "Greg/gregress not found; its rows will be marked skip"

if `require_comparators' & (!`have_reghdfe' | !`have_greg') {
    display as error "require_comparators=1 but reghdfe or Greg is unavailable"
    exit 499
}
global GTOOLS_BETA "I KNOW WHAT I AM DOING"
local tuning_mode `"`tuning_settings'"'
display as text "Manual benchmark tolerance/iteration settings: `tuning_mode'"
if `harmonize_settings' display as text ///
    "  common tolerance: `common_tolerance'; common maximum iterations: `common_maxiter'"

frame create __fmb_results                                           ///
    str24 problem_type str8 topology byte fe_dimensions               ///
    long base_n long n_observations long n_workers long n_firms       ///
    long n_counties byte controls int requested_threads               ///
    int stata_threads double effective_threads int repetition         ///
    byte sequence str10 command str12 thread_control str12 tuning_mode ///
    str8 status                                                       ///
    int rc double elapsed_seconds double n_estimation                  ///
    double fereg_native_seconds double peak_rss_mb                     ///
    double native_allocation_mb str32 memory_scope str80 note          ///
    strL command_line

frame __fmb_results: label variable problem_type          "Benchmark problem type"
frame __fmb_results: label variable topology              "Mobility topology"
frame __fmb_results: label variable fe_dimensions         "Number of absorbed fixed-effect dimensions"
frame __fmb_results: label variable base_n                "Unscaled requested observation count"
frame __fmb_results: label variable n_observations        "Actual observation count after scaling"
frame __fmb_results: label variable n_workers             "Number of simulated workers"
frame __fmb_results: label variable n_firms               "Number of simulated firms"
frame __fmb_results: label variable n_counties            "Number of simulated counties"
frame __fmb_results: label variable controls              "Number of continuous controls"
frame __fmb_results: label variable requested_threads     "Requested benchmark thread count"
frame __fmb_results: label variable stata_threads         "Stata processors after license and machine caps"
frame __fmb_results: label variable effective_threads     "Command-reported effective thread count"
frame __fmb_results: label variable repetition           "Recorded repetition number"
frame __fmb_results: label variable sequence             "Command position within the repetition"
frame __fmb_results: label variable command              "Estimator command"
frame __fmb_results: label variable thread_control       "How the command's thread count is controlled"
frame __fmb_results: label variable tuning_mode          "Default or harmonized tolerance/iteration settings"
frame __fmb_results: label variable status               "Run status: pass, fail, skip, or capped"
frame __fmb_results: label variable rc                   "Stata return code"
frame __fmb_results: label variable elapsed_seconds      "Elapsed wall time in seconds"
frame __fmb_results: label variable n_estimation         "Observations in the estimation sample"
frame __fmb_results: label variable fereg_native_seconds "Native fereg execution time in seconds"
frame __fmb_results: label variable peak_rss_mb          "Process peak RSS reported by fereg in MiB"
frame __fmb_results: label variable native_allocation_mb "Peak native allocation reported by fereg in MiB"
frame __fmb_results: label variable memory_scope         "Scope and availability of memory measurement"
frame __fmb_results: label variable note                 "Skip, failure, or measurement note"
frame __fmb_results: label variable command_line         "Full command used for the run"

global FMB_RESULTS __fmb_results
global FMB_HAVE_FEREG `have_fereg'
global FMB_HAVE_REGHDFE `have_reghdfe'
global FMB_HAVE_GREG `have_greg'
global FMB_HARMONIZE `harmonize_settings'
global FMB_COMMON_TOLERANCE `common_tolerance'
global FMB_COMMON_MAXITER `common_maxiter'

capture program drop __fmb_generate
program define __fmb_generate, rclass
    version 19.0
    args n problem_type case_seed controls

    clear
    set rng mt64
    set seed `case_seed'

    local periods 5
    local workers = `n' / `periods'
    local firms = max(50, ceil(`workers' / 20))
    local counties = max(10, ceil(`firms' / 20))
    local mobility .20
    local topology "dense"
    local fe_dimensions 1
    local absorb "worker"

    if `"`problem_type'"' == "one_fe" {
        local topology "n/a"
    }
    else if `"`problem_type'"' == "two_fe_dense" {
        local fe_dimensions 2
        local absorb "worker firm"
    }
    else if `"`problem_type'"' == "two_fe_sparse" {
        local topology "sparse"
        local mobility .02
        local fe_dimensions 2
        local absorb "worker firm"
    }
    else if `"`problem_type'"' == "three_fe_dense" {
        local fe_dimensions 3
        local absorb "worker firm county"
    }
    else if `"`problem_type'"' == "three_fe_sparse" {
        local topology "sparse"
        local mobility .02
        local fe_dimensions 3
        local absorb "worker firm county"
    }

    quietly set obs `n'
    generate long obs_id = _n
    generate long worker = ceil(_n / `periods')
    bysort worker: generate byte period = _n
    bysort worker (period): generate long firm =                    ///
        mod(7919*worker - 1, `firms') + 1 if _n == 1
    bysort worker (period): replace firm =                           ///
        cond(runiform() < `mobility',                                ///
            mod(104729*worker + 7919*period - 1, `firms') + 1,       ///
            firm[_n-1]) if _n > 1

    generate long county =                                          ///
        mod(firm + 17*worker + 1049*period - 1, `counties') + 1

    bysort worker: generate double __fmb_worker_fe =                 ///
        rnormal(0, .4) if _n == 1
    bysort worker: replace __fmb_worker_fe = __fmb_worker_fe[1]
    bysort firm: generate double __fmb_firm_fe =                     ///
        rnormal(0, .2) if _n == 1
    bysort firm: replace __fmb_firm_fe = __fmb_firm_fe[1]
    bysort county: generate double __fmb_county_fe =                 ///
        rnormal(0, .1) if _n == 1
    bysort county: replace __fmb_county_fe = __fmb_county_fe[1]

    generate double __fmb_xb = 0
    local xvars
    forvalues control = 1/`controls' {
        generate double x`control' = rnormal()
        local beta = cond(mod(`control', 2), .30/`control', -.20/`control')
        replace __fmb_xb = __fmb_xb + `beta'*x`control'
        local xvars `"`xvars' x`control'"'
    }
    local xvars = strtrim(`"`xvars'"')

    generate double y = 1 + __fmb_xb + __fmb_worker_fe + rnormal(0, .5)
    if `fe_dimensions' >= 2 replace y = y + __fmb_firm_fe
    if `fe_dimensions' == 3 replace y = y + __fmb_county_fe
    drop __fmb_worker_fe __fmb_firm_fe __fmb_county_fe __fmb_xb
    sort worker period

    label variable obs_id "Observation identifier"
    label variable worker "Worker identifier"
    label variable period "Within-worker period"
    label variable firm   "Firm identifier"
    label variable county "Observation-level county identifier"
    label variable y      "Simulated outcome"
    forvalues control = 1/`controls' {
        label variable x`control' "Simulated control `control'"
    }

    assert _N == `n'
    return scalar n_workers = `workers'
    return scalar n_firms = `firms'
    return scalar n_counties = `counties'
    return scalar fe_dimensions = `fe_dimensions'
    return scalar mobility = `mobility'
    return local topology `"`topology'"'
    return local absorb `"`absorb'"'
    return local xvars `"`xvars'"'
end

capture program drop __fmb_run_arm
program define __fmb_run_arm, rclass
    version 19.0
    args arm record repetition sequence

    local problem_type `"$FMB_PROBLEM"'
    local topology `"$FMB_TOPOLOGY"'
    local fe_dimensions = real("$FMB_FE_DIMENSIONS")
    local base_n = real("$FMB_BASE_N")
    local n = real("$FMB_N")
    local workers = real("$FMB_WORKERS")
    local firms = real("$FMB_FIRMS")
    local counties = real("$FMB_COUNTIES")
    local controls = real("$FMB_CONTROLS")
    local requested_threads = real("$FMB_REQUESTED_THREADS")
    local stata_threads = real("$FMB_STATA_THREADS")
    local absorb `"$FMB_ABSORB"'
    local xvars `"$FMB_XVARS"'
    local harmonize = real("$FMB_HARMONIZE")
    local common_tolerance = real("$FMB_COMMON_TOLERANCE")
    local common_maxiter = real("$FMB_COMMON_MAXITER")
    local tuning_mode = cond(`harmonize', "harmonized", "default")

    local available 1
    local thread_control "stata"
    local note
    if `"`arm'"' == "fereg" {
        local available = $FMB_HAVE_FEREG
        local thread_control "native"
        if `harmonize' {
            local command_line                                      ///
                `"fereg y `xvars', absorb(`absorb') dof(pairwise) threads(`requested_threads') tolerance(`common_tolerance') iterate(`common_maxiter')"'
        }
        else {
            local command_line                                      ///
                `"fereg y `xvars', absorb(`absorb') dof(pairwise) threads(`requested_threads')"'
        }
    }
    else if `"`arm'"' == "reghdfe" {
        local available = $FMB_HAVE_REGHDFE
        if `harmonize' {
            local command_line                                      ///
                `"reghdfe y `xvars', absorb(`absorb') dof(pairwise) tolerance(`common_tolerance') iterate(`common_maxiter')"'
        }
        else {
            local command_line                                      ///
                `"reghdfe y `xvars', absorb(`absorb') dof(pairwise)"'
        }
    }
    else if `"`arm'"' == "areg" {
        if `fe_dimensions' == 1 {
            local command_line `"areg y `xvars', absorb(worker)"'
        }
        else if `harmonize' {
            local command_line                                      ///
                `"areg y `xvars', absorb(`absorb') dfabsorb noabsorbtest tolerance(`common_tolerance') iterate(`common_maxiter') nolog"'
        }
        else {
            local command_line                                      ///
                `"areg y `xvars', absorb(`absorb') dfabsorb noabsorbtest nolog"'
        }
    }
    else if `"`arm'"' == "greg" {
        local available = $FMB_HAVE_GREG
        local thread_control "uncontrolled"
        if `harmonize' {
            local command_line                                      ///
                `"greg y `xvars', absorb(`absorb') hdfetol(`common_tolerance') maxiter(`common_maxiter') mata(GtoolsRegress)"'
        }
        else {
            local command_line                                      ///
                `"greg y `xvars', absorb(`absorb') mata(GtoolsRegress)"'
        }
    }
    else {
        display as error "unknown benchmark command: `arm'"
        exit 198
    }

    if !`available' {
        if `record' {
            frame post $FMB_RESULTS                                 ///
                (`"`problem_type'"') (`"`topology'"')              ///
                (`fe_dimensions') (`base_n') (`n') (`workers')       ///
                (`firms') (`counties') (`controls')                  ///
                (`requested_threads') (`stata_threads') (.)          ///
                (`repetition') (`sequence') (`"`arm'"')             ///
                (`"`thread_control'"') (`"`tuning_mode'"')       ///
                ("skip") (111) (.) (.)                              ///
                (.) (.) (.) ("not exposed by command")             ///
                ("dependency missing") (`"`command_line'"')
        }
        return scalar rc = 111
        return scalar skipped = 1
        exit
    }

    if `"`thread_control'"' == "stata" & ///
            `requested_threads' > `stata_threads' {
        if `record' {
            frame post $FMB_RESULTS                                 ///
                (`"`problem_type'"') (`"`topology'"')              ///
                (`fe_dimensions') (`base_n') (`n') (`workers')       ///
                (`firms') (`counties') (`controls')                  ///
                (`requested_threads') (`stata_threads') (.)          ///
                (`repetition') (`sequence') (`"`arm'"')             ///
                (`"`thread_control'"') (`"`tuning_mode'"')       ///
                ("capped") (.) (.) (.) (.) (.) (.)                 ///
                ("not exposed by command")                         ///
                ("omitted: requested threads exceed the Stata processor cap") ///
                (`"`command_line'"')
            display as text ///
                "OMIT  `problem_type' N=`n' threads=`requested_threads' `arm': Stata cap is `stata_threads'"
        }
        return scalar rc = 0
        return scalar skipped = 1
        exit
    }

    quietly set processors `stata_threads'
    timer clear 91
    timer on 91
    capture quietly `command_line'
    local command_rc = _rc
    timer off 91
    quietly timer list 91
    local elapsed_seconds = r(t91)
    timer clear 91

    local status = cond(`command_rc', "fail", "pass")
    local n_estimation .
    local effective_threads .
    local fereg_native_seconds .
    local peak_rss_mb .
    local native_allocation_mb .
    local memory_scope "not exposed by command"
    if `command_rc' {
        local note "command returned r(`command_rc')"
    }
    else if `"`arm'"' == "greg" {
        capture local n_estimation = r(N)
    }
    else {
        capture local n_estimation = e(N)
        local effective_threads = `stata_threads'
    }

    if !`command_rc' & `"`arm'"' == "fereg" {
        capture local effective_threads = e(fereg_threads)
        capture local fereg_native_seconds = el(e(fereg_runtime), 20, 2)
        capture local peak_rss_mb = e(fereg_peak_rss_bytes) / 1048576
        capture local native_allocation_mb =                         ///
            e(fereg_native_allocation_bytes) / 1048576
        local memory_scope "fereg process high-water"
        local note "memory is not comparator-equivalent"
    }

    if `record' {
        frame post $FMB_RESULTS                                     ///
            (`"`problem_type'"') (`"`topology'"')                  ///
            (`fe_dimensions') (`base_n') (`n') (`workers')           ///
            (`firms') (`counties') (`controls')                      ///
            (`requested_threads') (`stata_threads')                  ///
            (`effective_threads') (`repetition') (`sequence')        ///
            (`"`arm'"') (`"`thread_control'"')                    ///
            (`"`tuning_mode'"') (`"`status'"')                    ///
            (`command_rc') (`elapsed_seconds') (`n_estimation')      ///
            (`fereg_native_seconds') (`peak_rss_mb')                 ///
            (`native_allocation_mb') (`"`memory_scope'"')           ///
            (`"`note'"') (`"`command_line'"')

        if `command_rc' {
            display as error "FAIL  `problem_type' N=`n' threads=`requested_threads' `arm': r(`command_rc')"
        }
        else {
            display as result "RUN   `problem_type' N=`n' threads=`requested_threads' `arm': " ///
                %9.4f `elapsed_seconds' " seconds"
        }
    }
    return scalar rc = `command_rc'
    return scalar skipped = 0
end

frame create __fmb_data
frame change __fmb_data

local problem_index 0
foreach problem_type of local selected_problem_types {
    local ++problem_index
    local warmed_fereg 0
    local warmed_reghdfe = !`have_reghdfe'
    local warmed_areg 0
    local warmed_greg = !`have_greg'
    local size_index 0
    foreach base_n of local dataset_sizes {
        local ++size_index
        local n = 5 * ceil((real(`"`base_n'"') * `scale') / 5)
        local case_seed = `seed' + 1000*`problem_index' + `size_index'
        __fmb_generate `n' `problem_type' `case_seed' `controls'
        local workers = r(n_workers)
        local firms = r(n_firms)
        local counties = r(n_counties)
        local fe_dimensions = r(fe_dimensions)
        local topology `"`r(topology)'"'
        local absorb `"`r(absorb)'"'
        local xvars `"`r(xvars)'"'

        global FMB_PROBLEM `"`problem_type'"'
        global FMB_TOPOLOGY `"`topology'"'
        global FMB_FE_DIMENSIONS `fe_dimensions'
        global FMB_BASE_N `base_n'
        global FMB_N `n'
        global FMB_WORKERS `workers'
        global FMB_FIRMS `firms'
        global FMB_COUNTIES `counties'
        global FMB_CONTROLS `controls'
        global FMB_ABSORB `"`absorb'"'
        global FMB_XVARS `"`xvars'"'

        display as text _newline                                      ///
            "Dataset: `problem_type', N=`n', FE dimensions=`fe_dimensions', controls=`controls'"

        /* Every size enters the fixed-thread slice.  Only the medium size
           receives the remaining thread counts. */
        local cell_threads `"`fixed_threads'"'
        if `base_n' == `medium_dataset_size' {
            foreach requested_threads of local thread_counts {
                if `requested_threads' != `fixed_threads' {
                    local cell_threads `"`cell_threads' `requested_threads'"'
                }
            }
        }

        local thread_index 0
        foreach requested_threads of local cell_threads {
            local ++thread_index
            local stata_threads = min(real(`"`requested_threads'"'), ///
                c(processors_lic), c(processors_mach))
            global FMB_REQUESTED_THREADS `requested_threads'
            global FMB_STATA_THREADS `stata_threads'

            /* Warm each runnable command once per problem type, outside the
               recorded timings. */
            local arms "fereg reghdfe areg"
            if `requested_threads' == `fixed_threads'               ///
                local arms "fereg reghdfe areg greg"
            foreach arm of local arms {
                local warm_flag = `warmed_`arm''
                if !`warm_flag' {
                    quietly __fmb_run_arm `arm' 0 0 0
                    if !r(skipped) local warmed_`arm' 1
                }
            }

            local arm_count : word count `arms'
            forvalues repetition = 1/`repetitions' {
                local shift = mod(`problem_index' + `size_index' +       ///
                    `thread_index' + `repetition', `arm_count')
                forvalues sequence = 1/`arm_count' {
                    local arm_index = mod(`sequence' + `shift' - 1,     ///
                        `arm_count') + 1
                    local arm : word `arm_index' of `arms'
                    __fmb_run_arm `arm' 1 `repetition' `sequence'
                }
            }
        }
    }
}

frame change __fmb_results
sort problem_type n_observations requested_threads repetition sequence
format elapsed_seconds fereg_native_seconds %10.4f
format peak_rss_mb native_allocation_mb %12.2f

quietly save `"`output_stem'_runs.dta"', replace
quietly export delimited using `"`output_stem'_runs.csv"', replace

frame copy __fmb_results __fmb_summary, replace
frame change __fmb_summary
keep if status == "pass"
generate byte successful_runs = 1
collapse (median) median_seconds=elapsed_seconds                     ///
                  median_effective_threads=effective_threads         ///
                  median_peak_rss_mb=peak_rss_mb                     ///
                  median_native_allocation_mb=native_allocation_mb   ///
         (sum) successful_runs,                                      ///
    by(problem_type topology fe_dimensions base_n n_observations     ///
       n_workers n_firms n_counties controls requested_threads       ///
       stata_threads command thread_control tuning_mode)

label variable problem_type                "Benchmark problem type"
label variable topology                    "Mobility topology"
label variable fe_dimensions               "Absorbed fixed-effect dimensions"
label variable base_n                      "Unscaled observation count"
label variable n_observations              "Actual observation count"
label variable n_workers                   "Number of workers"
label variable n_firms                     "Number of firms"
label variable n_counties                  "Number of counties"
label variable controls                    "Number of controls"
label variable requested_threads           "Requested thread count"
label variable stata_threads               "Capped Stata processor count"
label variable command                     "Estimator command"
label variable thread_control              "Thread-control mechanism"
label variable tuning_mode                 "Default or harmonized tolerance/iteration settings"
label variable median_seconds              "Median elapsed time in seconds"
label variable median_effective_threads    "Median effective thread count"
label variable median_peak_rss_mb          "Median fereg process peak RSS in MiB"
label variable median_native_allocation_mb "Median fereg native allocation in MiB"
label variable successful_runs             "Successful recorded runs"

generate byte panel = 1 if problem_type == "two_fe_dense"
replace panel = 2 if problem_type == "two_fe_sparse"
replace panel = 3 if problem_type == "three_fe_sparse"
replace panel = 4 if problem_type == "one_fe"
replace panel = 5 if problem_type == "three_fe_dense"
label define fmb_panel 1 "Two-way dense" 2 "Two-way sparse"       ///
    3 "Three-FE sparse" 4 "One FE" 5 "Three-FE dense"
label values panel fmb_panel
label variable panel "Ordered problem-type figure panel"

sort panel requested_threads n_observations command
quietly save `"`output_stem'_summary.dta"', replace
quietly export delimited using `"`output_stem'_summary.csv"', replace

if `make_figures' {
    local tuning_subtitle "Each command uses its default tolerance and iteration settings"
    if `harmonize_settings' {
        local tuning_subtitle ///
            "Harmonized tolerance=`common_tolerance'; max iterations=`common_maxiter'"
    }

    local size_plots
    local legend_order
    local legend_labels
    local plot_number 0
    foreach arm in fereg reghdfe areg greg {
        quietly count if requested_threads == `fixed_threads' &    ///
            panel <= 3 & command == `"`arm'"' & median_seconds > 0
        if r(N) {
            local ++plot_number
            local line_color "midblue"
            local line_pattern "solid"
            local marker_symbol "circle"
            local arm_label "fereg"
            if `"`arm'"' == "reghdfe" {
                local line_color "cranberry"
                local line_pattern "dash"
                local marker_symbol "triangle"
                local arm_label "reghdfe"
            }
            else if `"`arm'"' == "areg" {
                local line_color "teal"
                local line_pattern "shortdash"
                local marker_symbol "square"
                local arm_label "areg"
            }
            else if `"`arm'"' == "greg" {
                local line_color "purple"
                local line_pattern "dot"
                local marker_symbol "diamond"
                local arm_label "Greg"
            }
            local size_plots `"`size_plots' (connected median_seconds n_observations if requested_threads == `fixed_threads' & panel <= 3 & command == "`arm'" & median_seconds > 0, sort lcolor(`line_color') lpattern(`line_pattern') lwidth(medthick) mcolor(`line_color') msymbol(`marker_symbol') msize(small))"'
            local legend_order `"`legend_order' `plot_number'"'
            local legend_labels `"`legend_labels' label(`plot_number' "`arm_label'")"'
        }
    }

    quietly levelsof n_observations if ///
        requested_threads == `fixed_threads' & panel <= 3, local(size_ticks)
    local size_labels
    foreach size of local size_ticks {
        local tick_label : display %12.0fc `size'
        if `size' >= 1000000 {
            local size_value = `size' / 1000000
            local tick_label : display %9.1f `size_value'
            if `size_value' == floor(`size_value') ///
                local tick_label : display %9.0f `size_value'
            local tick_label = strtrim(`"`tick_label'"') + "m"
        }
        else if `size' >= 1000 {
            local size_value = `size' / 1000
            local tick_label : display %9.1f `size_value'
            if `size_value' == floor(`size_value') ///
                local tick_label : display %9.0f `size_value'
            local tick_label = strtrim(`"`tick_label'"') + "k"
        }
        local size_labels `"`size_labels' `size' "`tick_label'""'
    }

    quietly summarize median_seconds if                            ///
        requested_threads == `fixed_threads' & panel <= 3 &        ///
        median_seconds > 0, meanonly
    local size_minimum_seconds = r(min)
    local size_maximum_seconds = r(max)
    local size_first_exponent = floor(log10(`size_minimum_seconds'))
    local size_last_exponent  = ceil(log10(`size_maximum_seconds'))
    local size_time_ticks
    forvalues exponent = `size_first_exponent'/`size_last_exponent' {
        foreach multiplier in 1 2 5 {
            local time_tick = `multiplier' * (10 ^ `exponent')
            if `time_tick' >= 0.5 * `size_minimum_seconds' &        ///
               `time_tick' <= 1.5 * `size_maximum_seconds'         ///
                local size_time_ticks `"`size_time_ticks' `time_tick'"'
        }
    }

    if `"`size_plots'"' != "" {
        twoway `size_plots',                                      ///
            by(panel, cols(3) compact noiytitle noixtitle          ///
                imargin(small) iscale(*.85)                        ///
                title("Elapsed time by dataset size at `fixed_threads' cores", size(medsmall)) ///
                subtitle("`tuning_subtitle'; logarithmic y-axis", size(small)) ///
                b1title("Observations", size(small))               ///
                l1title("Median elapsed time (seconds)", size(small)) ///
                note("Capped Stata arms are omitted; Greg has no per-call core control.", size(vsmall)) ///
                graphregion(color(white) margin(small)))           ///
            xlabel(`size_labels', angle(0) labsize(vsmall))       ///
            yscale(log) ylabel(`size_time_ticks', grid labsize(vsmall) format(%9.3g)) ///
            plotregion(margin(vsmall)) ytitle("") xtitle("")     ///
            legend(order(`legend_order') `legend_labels' cols(1) ///
                position(3) ring(1) size(small) symxsize(6)       ///
                keygap(tiny) rowgap(tiny)                         ///
                region(lstyle(none) margin(vsmall)))              ///
            xsize(14) ysize(5.5) name(__fmb_size, replace)
        graph export `"`output_stem'_time_by_size.png"',         ///
            name(__fmb_size) width(2600) replace
    }

    local thread_plots
    local legend_order
    local legend_labels
    local plot_number 0
    foreach arm in fereg reghdfe areg {
        quietly count if base_n == `medium_dataset_size' &        ///
            panel <= 3 & command == `"`arm'"' & median_seconds > 0
        if r(N) {
            local ++plot_number
            local line_color "midblue"
            local line_pattern "solid"
            local marker_symbol "circle"
            local arm_label "fereg"
            if `"`arm'"' == "reghdfe" {
                local line_color "cranberry"
                local line_pattern "dash"
                local marker_symbol "triangle"
                local arm_label "reghdfe"
            }
            else if `"`arm'"' == "areg" {
                local line_color "teal"
                local line_pattern "shortdash"
                local marker_symbol "square"
                local arm_label "areg"
            }
            local thread_plots `"`thread_plots' (connected median_seconds requested_threads if base_n == `medium_dataset_size' & panel <= 3 & command == "`arm'" & median_seconds > 0, sort lcolor(`line_color') lpattern(`line_pattern') lwidth(medthick) mcolor(`line_color') msymbol(`marker_symbol') msize(small))"'
            local legend_order `"`legend_order' `plot_number'"'
            local legend_labels `"`legend_labels' label(`plot_number' "`arm_label'")"'
        }
    }

    local medium_n = 5 * ceil((`medium_dataset_size' * `scale') / 5)
    quietly summarize median_seconds if base_n == `medium_dataset_size' & ///
        panel <= 3 & command != "greg" & median_seconds > 0, meanonly
    local thread_minimum_seconds = r(min)
    local thread_maximum_seconds = r(max)
    local thread_first_exponent = floor(log10(`thread_minimum_seconds'))
    local thread_last_exponent  = ceil(log10(`thread_maximum_seconds'))
    local thread_time_ticks
    forvalues exponent = `thread_first_exponent'/`thread_last_exponent' {
        foreach multiplier in 1 2 5 {
            local time_tick = `multiplier' * (10 ^ `exponent')
            if `time_tick' >= 0.5 * `thread_minimum_seconds' &      ///
               `time_tick' <= 1.5 * `thread_maximum_seconds'       ///
                local thread_time_ticks `"`thread_time_ticks' `time_tick'"'
        }
    }
    if `"`thread_plots'"' != "" {
        twoway `thread_plots',                                    ///
            by(panel, cols(3) compact noiytitle noixtitle          ///
                imargin(small) iscale(*.85)                        ///
                title("Elapsed time by cores at N=`medium_n'", size(medsmall)) ///
                subtitle("`tuning_subtitle'; logarithmic y-axis", size(small)) ///
                b1title("Requested cores", size(small))            ///
                l1title("Median elapsed time (seconds)", size(small)) ///
                note("Stata-controlled observations above the local processor cap are omitted; Greg has no per-call thread control.", size(vsmall)) ///
                graphregion(color(white) margin(small)))           ///
            xlabel(`thread_counts', labsize(small))               ///
            yscale(log) ylabel(`thread_time_ticks', grid labsize(vsmall) format(%9.3g)) ///
            plotregion(margin(vsmall)) ytitle("") xtitle("")     ///
            legend(order(`legend_order') `legend_labels' cols(1) ///
                position(3) ring(1) size(small) symxsize(6)       ///
                keygap(tiny) rowgap(tiny)                         ///
                region(lstyle(none) margin(vsmall)))              ///
            xsize(14) ysize(5.5) name(__fmb_cores, replace)
        graph export `"`output_stem'_time_by_cores.png"',        ///
            name(__fmb_cores) width(2600) replace
    }
}

quietly levelsof command if !missing(median_peak_rss_mb), local(memory_commands)
local comparable_memory_commands : word count `memory_commands'
if `comparable_memory_commands' < 2 {
    display as text _newline                                           ///
        "Memory figures were not generated: Stata exposes no comparable per-command peak RSS for the comparator commands."
}

frame change __fmb_results
quietly count
local total_runs = r(N)
quietly count if status == "pass"
local passed_runs = r(N)
quietly count if status == "fail"
local failed_runs = r(N)
quietly count if status == "skip"
local skipped_runs = r(N)
quietly count if status == "capped"
local capped_runs = r(N)

display as text _newline "Manual benchmark summary"
display as text "  recorded runs: " as result `total_runs'
display as text "  passed:        " as result `passed_runs'
display as text "  failed:        " as result `failed_runs'
display as text "  skipped:       " as result `skipped_runs'
display as text "  capped/omitted:" as result `capped_runs'
tabulate status command, missing
capture noisily tabstat elapsed_seconds if status == "pass",          ///
    by(command) statistics(n median mean min max) columns(statistics)
display as text "  run data:      " as result `"`output_stem'_runs.dta"'
display as text "  summary data:  " as result `"`output_stem'_summary.dta"'

// capture graph drop __fmb_size __fmb_cores
frame drop __fmb_data
frame drop __fmb_summary
capture program drop __fmb_generate
capture program drop __fmb_run_arm
foreach global_name in FMB_RESULTS FMB_HAVE_FEREG FMB_HAVE_REGHDFE     ///
    FMB_HAVE_GREG FMB_HARMONIZE FMB_COMMON_TOLERANCE                  ///
    FMB_COMMON_MAXITER FMB_PROBLEM FMB_TOPOLOGY FMB_FE_DIMENSIONS     ///
    FMB_BASE_N FMB_N FMB_WORKERS FMB_FIRMS FMB_COUNTIES FMB_CONTROLS ///
    FMB_REQUESTED_THREADS FMB_STATA_THREADS FMB_ABSORB FMB_XVARS {
    capture macro drop `global_name'
}
if `"`original_gtools_beta'"' == "" capture macro drop GTOOLS_BETA
else global GTOOLS_BETA `"`original_gtools_beta'"'
set processors `original_processors'

if `failed_runs' {
    display as error "FEREG_MANUAL_BENCHMARKS|FAIL|runs=`total_runs'|passed=`passed_runs'|failed=`failed_runs'|skipped=`skipped_runs'|capped=`capped_runs'|tuning=`tuning_mode'"
    if `strict' exit 459
}
else if `skipped_runs' {
    display as text "FEREG_MANUAL_BENCHMARKS|INCOMPLETE|runs=`total_runs'|passed=`passed_runs'|failed=0|skipped=`skipped_runs'|capped=`capped_runs'|tuning=`tuning_mode'"
}
else {
    display as result "FEREG_MANUAL_BENCHMARKS|PASS|runs=`total_runs'|passed=`passed_runs'|failed=0|skipped=0|capped=`capped_runs'|tuning=`tuning_mode'"
}
