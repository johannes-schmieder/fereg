/*
    Straightforward manual performance benchmark for feppml.

    The file simulates balanced directed trade panels and compares end-to-end
    elapsed time for feppml and ppmlhdfe across country counts and thread
    counts.  Data generation and one warm-up call per command are outside the
    recorded timings.  Command order rotates across repetitions.

    Run-level and median-summary datasets and CSV files are written to
    output_directory.  Two optional PNG figures show size scaling at a fixed
    thread count and thread scaling at a fixed country count.

    This is an accessible diagnostic benchmark, not release evidence.  Run
    tests/manual/manual_validation_ppml.do separately for correctness checks.

    The file exercises the public feppml command.
*/

version 18.0
clear all
set more off
set linesize 255
capture graph drop _all

/* ---------------------------- User settings ---------------------------- */
local country_counts    "25 50 100" // Directed panels contain C*(C-1)*T rows.
local periods           4           // Number of years per country pair.
local first_year        2018        // First simulated calendar year.
local thread_counts     "1 2 4 8"   // Threads used by both commands.
local fixed_threads     8           // Thread count shown in the size figure.
local medium_countries  50          // Country count shown in the thread figure.
local repetitions      3           // Recorded warm repetitions per cell.
local seed             20260901    // Base random-number seed.
local output_directory "."         // Directory for data and figures.
local make_figures     1           // Export PNG figures (0: datasets only).
local strict           1           // Return r(459) after outputs on any failure.
/* ----------------------------------------------------------------------- */

if c(stata_version) < 18 {
    display as error "manual_benchmarks_ppml.do requires Stata 18 or newer"
    exit 9
}
if `periods' < 2 | `periods' != floor(`periods') {
    display as error "periods must be an integer of at least two"
    exit 198
}
if `repetitions' < 1 | `repetitions' != floor(`repetitions') {
    display as error "repetitions must be a positive integer"
    exit 198
}
if !inlist(`make_figures', 0, 1) | !inlist(`strict', 0, 1) {
    display as error "make_figures and strict must be zero or one"
    exit 198
}

local medium_found 0
foreach countries of local country_counts {
    if missing(real(`"`countries'"')) | real(`"`countries'"') < 4 | ///
       real(`"`countries'"') != floor(real(`"`countries'"')) {
        display as error "country_counts must contain integers of at least four"
        exit 198
    }
    if real(`"`countries'"') == `medium_countries' local medium_found 1
}
if !`medium_found' {
    display as error "medium_countries must be listed in country_counts"
    exit 198
}

local processor_cap = min(c(processors_lic), c(processors_mach))
local fixed_found 0
foreach requested_threads of local thread_counts {
    if missing(real(`"`requested_threads'"')) | ///
       real(`"`requested_threads'"') < 1 | ///
       real(`"`requested_threads'"') > `processor_cap' | ///
       real(`"`requested_threads'"') != floor(real(`"`requested_threads'"')) {
        display as error ///
            "thread_counts must contain integers from one through the local processor cap (`processor_cap')"
        exit 198
    }
    if real(`"`requested_threads'"') == `fixed_threads' local fixed_found 1
}
if !`fixed_found' {
    display as error "fixed_threads must be listed in thread_counts"
    exit 198
}

local feppml_command "feppml"

capture findfile `feppml_command'.ado
if _rc {
    display as error "`feppml_command' is not installed or not on the ado-path"
    exit 111
}
local feppml_path `"`r(fn)'"'

capture findfile ppmlhdfe.ado
if _rc {
    display as error "ppmlhdfe is required for this manual benchmark"
    exit 111
}
local ppmlhdfe_path `"`r(fn)'"'

capture mkdir `"`output_directory'"'
local output_stem `"`output_directory'/manual_benchmarks_ppml"'
local original_processors = c(processors)

display as text "Manual feppml trade-data benchmark"
display as text "  Stata:       `c(stata_version)' `c(flavor)'"
display as text "  candidate:   `feppml_command' (`feppml_path')"
display as text "  comparator:  ppmlhdfe (`ppmlhdfe_path')"
display as text "  countries:   `country_counts'"
display as text "  periods:     `periods'"
display as text "  threads:     `thread_counts'"
display as text "  repetitions: `repetitions'"

frame create __fpmb_results                                         ///
    int countries int periods long n_observations int requested_threads ///
    int repetition byte sequence str10 command str8 status int rc     ///
    double elapsed_seconds double n_estimation double effective_threads ///
    strL command_line

frame __fpmb_results: label variable countries          "Number of countries"
frame __fpmb_results: label variable periods            "Number of years"
frame __fpmb_results: label variable n_observations     "Simulated observations"
frame __fpmb_results: label variable requested_threads  "Requested thread count"
frame __fpmb_results: label variable repetition        "Recorded repetition"
frame __fpmb_results: label variable sequence          "Command order within repetition"
frame __fpmb_results: label variable command           "Estimator command"
frame __fpmb_results: label variable status            "Run status: pass or fail"
frame __fpmb_results: label variable rc                "Stata return code"
frame __fpmb_results: label variable elapsed_seconds   "Elapsed wall time in seconds"
frame __fpmb_results: label variable n_estimation      "Estimation-sample observations"
frame __fpmb_results: label variable effective_threads "Reported or configured threads"
frame __fpmb_results: label variable command_line      "Full command used"

local country_index 0
foreach countries of local country_counts {
    local ++country_index

    /* Generate one trade panel per country count, outside all timings. */
    clear
    set rng mt64
    set seed `=`seed' + 1009 * `countries''
    set obs `countries'
    generate int exporter = _n
    expand `countries'
    bysort exporter: generate int importer = _n
    drop if exporter == importer

    expand `periods'
    bysort exporter importer: generate int year = `first_year' + _n - 1
    egen long pair_id = group(exporter importer)

    bysort pair_id: generate double log_distance = ///
        log(300 + 9700 * runiform()) if _n == 1
    bysort pair_id: replace log_distance = log_distance[1]
    bysort pair_id: generate byte common_border = ///
        runiform() < .10 if _n == 1
    bysort pair_id: replace common_border = common_border[1]
    generate byte trade_agreement = ///
        year >= `first_year' + `periods' - 2 & ///
        mod(exporter + 2 * importer, 7) == 0

    bysort exporter year: generate double exporter_year_fe = ///
        rnormal(0, .25) if _n == 1
    bysort exporter year: replace exporter_year_fe = exporter_year_fe[1]
    bysort importer year: generate double importer_year_fe = ///
        rnormal(0, .25) if _n == 1
    bysort importer year: replace importer_year_fe = importer_year_fe[1]

    generate double eta = 6.0 - .55 * log_distance + ///
        .30 * common_border + .40 * trade_agreement + ///
        exporter_year_fe + importer_year_fe
    generate long trade = rpoisson(exp(eta))
    sort exporter importer year

    local n_observations = _N
    assert `n_observations' == `countries' * (`countries' - 1) * `periods'

    foreach requested_threads of local thread_counts {
        quietly set processors `requested_threads'

        local candidate_command                               ///
            `"`feppml_command' trade log_distance common_border trade_agreement, absorb(exporter#year importer#year) vce(cluster pair_id) separation(fe ir) threads(`requested_threads')"'
        local comparator_command                              ///
            `"ppmlhdfe trade log_distance common_border trade_agreement, absorb(exporter#year importer#year) vce(cluster pair_id)"'

        /* Warm command and solver paths outside recorded timings. */
        capture quietly `candidate_command'
        local warm_candidate_rc = _rc
        if `warm_candidate_rc' {
            display as error ///
                "`feppml_command' warm-up failed at C=`countries', threads=`requested_threads' with r(`warm_candidate_rc')"
            quietly set processors `original_processors'
            frame drop __fpmb_results
            exit `warm_candidate_rc'
        }
        capture quietly `comparator_command'
        local warm_comparator_rc = _rc
        if `warm_comparator_rc' {
            display as error ///
                "ppmlhdfe warm-up failed at C=`countries', threads=`requested_threads' with r(`warm_comparator_rc')"
            quietly set processors `original_processors'
            frame drop __fpmb_results
            exit `warm_comparator_rc'
        }

        forvalues repetition = 1/`repetitions' {
            local candidate_first = mod(`country_index' + ///
                `requested_threads' + `repetition', 2)
            if `candidate_first' local arms "feppml ppmlhdfe"
            else local arms "ppmlhdfe feppml"

            local sequence 0
            foreach arm of local arms {
                local ++sequence
                if `"`arm'"' == "feppml" {
                    local command_line `"`candidate_command'"'
                }
                else local command_line `"`comparator_command'"'

                timer clear 91
                timer on 91
                capture quietly `command_line'
                local run_rc = _rc
                timer off 91
                quietly timer list
                local elapsed_seconds = r(t91)
                timer clear 91

                local status "pass"
                local n_estimation .
                local effective_threads .
                if `run_rc' {
                    local status "fail"
                }
                else {
                    local n_estimation = e(N)
                    if `"`arm'"' == "feppml" {
                        capture local effective_threads = e(feppml_threads)
                    }
                    else local effective_threads = `requested_threads'
                }

                frame post __fpmb_results                           ///
                    (`countries') (`periods') (`n_observations')     ///
                    (`requested_threads') (`repetition') (`sequence') ///
                    (`"`arm'"') (`"`status'"') (`run_rc')        ///
                    (`elapsed_seconds') (`n_estimation')             ///
                    (`effective_threads') (`"`command_line'"')

                if `run_rc' display as error ///
                    "FAIL  C=`countries' threads=`requested_threads' repetition=`repetition' `arm' r(`run_rc')"
                else display as text ///
                    "PASS  C=`countries' N=`n_observations' threads=`requested_threads' repetition=`repetition' `arm': " ///
                    as result %9.4f `elapsed_seconds' as text " seconds"
            }
        }
    }
}

quietly set processors `original_processors'
frame change __fpmb_results
sort countries requested_threads repetition sequence
format elapsed_seconds %10.4f
format effective_threads %9.0f

save `"`output_stem'_runs.dta"', replace
export delimited using `"`output_stem'_runs.csv"', replace

preserve
keep if status == "pass"
collapse (median) median_seconds=elapsed_seconds ///
         (median) median_effective_threads=effective_threads ///
         (count) successful_runs=elapsed_seconds, ///
    by(countries periods n_observations requested_threads command)
label variable median_seconds           "Median elapsed time in seconds"
label variable median_effective_threads "Median effective thread count"
label variable successful_runs          "Successful recorded runs"
sort countries requested_threads command
save `"`output_stem'_summary.dta"', replace
export delimited using `"`output_stem'_summary.csv"', replace

if `make_figures' {
    twoway                                                     ///
        (connected median_seconds n_observations               ///
            if command == "feppml" & requested_threads == `fixed_threads', ///
            sort msymbol(circle) lpattern(solid))              ///
        (connected median_seconds n_observations               ///
            if command == "ppmlhdfe" & requested_threads == `fixed_threads', ///
            sort msymbol(triangle) lpattern(dash)),             ///
        yscale(log) xscale(log)                                 ///
        xlabel(, format(%12.0fc) labsize(small))                ///
        ylabel(, format(%9.3g) labsize(small))                  ///
        xtitle("Observations") ytitle("Median elapsed time (seconds)") ///
        title("PPML trade benchmark by dataset size")          ///
        subtitle("`fixed_threads' threads; logarithmic axes")  ///
        legend(order(1 "feppml" 2 "ppmlhdfe") rows(1))       ///
        graphregion(color(white)) name(fpmb_size, replace)
    graph export `"`output_stem'_time_by_size.png"', ///
        name(fpmb_size) width(1800) replace

    twoway                                                     ///
        (connected median_seconds requested_threads            ///
            if command == "feppml" & countries == `medium_countries', ///
            sort msymbol(circle) lpattern(solid))              ///
        (connected median_seconds requested_threads            ///
            if command == "ppmlhdfe" & countries == `medium_countries', ///
            sort msymbol(triangle) lpattern(dash)),             ///
        yscale(log)                                             ///
        xlabel(`thread_counts', labsize(small))                 ///
        ylabel(, format(%9.3g) labsize(small))                  ///
        xtitle("Threads") ytitle("Median elapsed time (seconds)") ///
        title("PPML trade benchmark by thread count")          ///
        subtitle("`medium_countries' countries; logarithmic y-axis") ///
        legend(order(1 "feppml" 2 "ppmlhdfe") rows(1))       ///
        graphregion(color(white)) name(fpmb_threads, replace)
    graph export `"`output_stem'_time_by_threads.png"', ///
        name(fpmb_threads) width(1800) replace
}
restore

quietly count
local recorded_runs = r(N)
quietly count if status == "pass"
local passed_runs = r(N)
quietly count if status == "fail"
local failed_rows = r(N)

display as text _newline "Manual PPML benchmark summary"
display as text "  recorded runs: " as result `recorded_runs'
display as text "  passed runs:   " as result `passed_runs'
display as text "  failed runs:   " as result `failed_rows'
display as text `"  run data:      `output_stem'_runs.dta"'
display as text `"  summary data:  `output_stem'_summary.dta"'

if `failed_rows' {
    list countries requested_threads repetition command rc ///
        if status == "fail", noobs abbreviate(24)
    display as error ///
        "FEPPML_MANUAL_BENCHMARK|FAIL|runs=`recorded_runs'|failed=`failed_rows'"
    if `strict' exit 459
}
else display as result ///
    "FEPPML_MANUAL_BENCHMARK|PASS|runs=`recorded_runs'|failed=0"
