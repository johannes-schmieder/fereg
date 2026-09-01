/*
    Straightforward manual validation for feppml.

    The file simulates a directed country-pair trade panel and compares
    feppml with ppmlhdfe.  It checks the estimation sample, selected slope
    coefficients, clustered standard errors, convergence, and fitted-mean
    publication.  The tolerances are diagnostic and do not replace the
    repository's frozen PPML qualification gates.

    Requirements:
      - Stata 18 or newer
      - feppml and ppmlhdfe on the ado-path
      - a fresh Stata session after installing or replacing the native plugin

    The file uses public feppml by default.  Developers working from a
    pre-activation source can run the same test against the private adapter
    with

        global FEREG_PPML_TEST_COMMAND _fereg_ppml
        do tests/manual/manual_validation_ppml.do
*/

version 18.0
clear all
set more off
set linesize 255

/* ---------------------------- User settings ---------------------------- */
local countries       40          // Number of simulated trading countries.
local periods         4           // Number of calendar years.
local first_year      2018        // First simulated calendar year.
local seed            20260901    // Random-number seed.
local threads         1           // Native threads requested from feppml.
local coefficient_tol 1e-6        // Maximum absolute slope difference.
local se_tol          1e-5        // Maximum absolute standard-error difference.
/* ----------------------------------------------------------------------- */

if c(stata_version) < 18 {
    display as error "manual_validation_ppml.do requires Stata 18 or newer"
    exit 9
}
if `countries' < 4 | `countries' != floor(`countries') {
    display as error "countries must be an integer of at least four"
    exit 198
}
if `periods' < 2 | `periods' != floor(`periods') {
    display as error "periods must be an integer of at least two"
    exit 198
}
if `threads' < 1 | `threads' > 64 | `threads' != floor(`threads') {
    display as error "threads must be an integer from one through 64"
    exit 198
}
if `coefficient_tol' <= 0 | `se_tol' <= 0 {
    display as error "comparison tolerances must be positive"
    exit 198
}

local feppml_command `"$FEREG_PPML_TEST_COMMAND"'
if strtrim(`"`feppml_command'"') == "" local feppml_command "feppml"

capture findfile `feppml_command'.ado
if _rc {
    display as error "`feppml_command' is not installed or not on the ado-path"
    exit 111
}
local feppml_path `"`r(fn)'"'

capture findfile ppmlhdfe.ado
if _rc {
    display as error "ppmlhdfe is required for this manual comparison"
    exit 111
}
local ppmlhdfe_path `"`r(fn)'"'

display as text "Manual feppml trade-data validation"
display as text "  Stata:       `c(stata_version)' `c(flavor)'"
display as text "  candidate:   `feppml_command' (`feppml_path')"
display as text "  comparator:  ppmlhdfe (`ppmlhdfe_path')"
display as text "  countries:   `countries'"
display as text "  periods:     `periods'"
display as text "  threads:     `threads'"

/* Simulate a balanced directed trade panel. */
set rng mt64
set seed `seed'
set obs `countries'
generate int exporter = _n
expand `countries'
bysort exporter: generate int importer = _n
drop if exporter == importer

expand `periods'
bysort exporter importer: generate int year = `first_year' + _n - 1
egen long pair_id = group(exporter importer)

/* Bilateral geography is fixed over time. */
bysort pair_id: generate double log_distance = ///
    log(300 + 9700 * runiform()) if _n == 1
bysort pair_id: replace log_distance = log_distance[1]
bysort pair_id: generate byte common_border = runiform() < .10 if _n == 1
bysort pair_id: replace common_border = common_border[1]

/* A subset of pairs enters a trade agreement in the final two years. */
generate byte trade_agreement = year >= `first_year' + `periods' - 2 & ///
    mod(exporter + 2 * importer, 7) == 0

/* Exporter-year and importer-year heterogeneity. */
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

order exporter importer year pair_id trade log_distance common_border ///
    trade_agreement
sort exporter importer year
assert _N == `countries' * (`countries' - 1) * `periods'

/* Warm command loading and native initialization outside the timings. */
capture quietly ppmlhdfe trade log_distance common_border trade_agreement, ///
    absorb(exporter#year importer#year) vce(cluster pair_id)
local warm_ppmlhdfe_rc = _rc
if `warm_ppmlhdfe_rc' {
    display as error "ppmlhdfe warm-up failed with r(`warm_ppmlhdfe_rc')"
    exit `warm_ppmlhdfe_rc'
}
capture noisily `feppml_command' trade log_distance common_border ///
    trade_agreement, absorb(exporter#year importer#year) ///
    vce(cluster pair_id) separation(fe ir) threads(`threads')
local warm_feppml_rc = _rc
if `warm_feppml_rc' {
    display as error "`feppml_command' warm-up failed with r(`warm_feppml_rc')"
    if `warm_feppml_rc' == 498 & `"`feppml_command'"' == "feppml" {
        display as error ///
            "this source has fail-closed public feppml; use the hidden adapter only for pre-activation development"
    }
    exit `warm_feppml_rc'
}

tempvar comparator_sample candidate_sample saved_mu predicted_mu mu_difference
tempname comparator_b candidate_b comparator_se candidate_se

timer clear 91
timer on 91
quietly ppmlhdfe trade log_distance common_border trade_agreement, ///
    absorb(exporter#year importer#year) vce(cluster pair_id)
timer off 91
quietly timer list
local comparator_seconds = r(t91)
timer clear 91

generate byte `comparator_sample' = e(sample)
matrix `comparator_b' = ///
    (_b[log_distance], _b[common_border], _b[trade_agreement])
matrix `comparator_se' = ///
    (_se[log_distance], _se[common_border], _se[trade_agreement])
local comparator_N = e(N)

timer on 92
quietly `feppml_command' trade log_distance common_border ///
    trade_agreement, absorb(exporter#year importer#year) ///
    vce(cluster pair_id) separation(fe ir) threads(`threads')
timer off 92
quietly timer list
local candidate_seconds = r(t92)
timer clear 92

/* Exercise retained means and public prediction outside the timing. */
quietly `feppml_command' trade log_distance common_border ///
    trade_agreement, absorb(exporter#year importer#year) ///
    vce(cluster pair_id) separation(fe ir) threads(`threads') ///
    mu(`saved_mu')
generate byte `candidate_sample' = e(sample)
matrix `candidate_b' = ///
    (_b[log_distance], _b[common_border], _b[trade_agreement])
matrix `candidate_se' = ///
    (_se[log_distance], _se[common_border], _se[trade_agreement])
local candidate_N = e(N)
local candidate_converged = e(converged)

quietly predict double `predicted_mu', mu
quietly count if `candidate_sample' != `comparator_sample'
local sample_mismatch = r(N)

mata:
st_numscalar("__fpmv_b_difference", max(abs(
    st_matrix(st_local("candidate_b")) :-
    st_matrix(st_local("comparator_b")))))
st_numscalar("__fpmv_se_difference", max(abs(
    st_matrix(st_local("candidate_se")) :-
    st_matrix(st_local("comparator_se")))))
end

scalar __fpmv_mu_difference = 0
generate double `mu_difference' = ///
    abs(`predicted_mu' - `saved_mu') if `candidate_sample'
quietly summarize `mu_difference', meanonly
scalar __fpmv_mu_difference = r(max)

display as text _newline "Validation results"
display as text "  observations:                  " as result %12.0f `candidate_N'
display as text "  sample-membership mismatches: " as result %12.0f `sample_mismatch'
display as text "  maximum coefficient difference:" as result %11.4e scalar(__fpmv_b_difference)
display as text "  maximum standard-error difference:" as result %8.4e scalar(__fpmv_se_difference)
display as text "  saved/predicted mean difference:" as result %9.4e scalar(__fpmv_mu_difference)
display as text "  feppml elapsed seconds:        " as result %12.4f `candidate_seconds'
display as text "  ppmlhdfe elapsed seconds:      " as result %12.4f `comparator_seconds'

assert `candidate_converged' == 1
assert `candidate_N' == `comparator_N'
assert `sample_mismatch' == 0
assert scalar(__fpmv_b_difference) <= `coefficient_tol'
assert scalar(__fpmv_se_difference) <= `se_tol'
assert scalar(__fpmv_mu_difference) <= 1e-10
assert `predicted_mu' > 0 & `predicted_mu' < . if `candidate_sample'
assert missing(`predicted_mu') if !`candidate_sample'

matrix colnames `candidate_b' = log_distance common_border trade_agreement
matrix colnames `comparator_b' = log_distance common_border trade_agreement
matrix rownames `candidate_b' = feppml
matrix rownames `comparator_b' = ppmlhdfe
matrix list `candidate_b', format(%12.6f)
matrix list `comparator_b', format(%12.6f)

scalar drop __fpmv_b_difference __fpmv_se_difference ///
    __fpmv_mu_difference
display as result ///
    "FEPPML_MANUAL_VALIDATION|PASS|N=`candidate_N'|sample_mismatch=0"
