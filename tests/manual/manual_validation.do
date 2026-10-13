/*
    Referee-facing validation suite for fereg.

    Requirements:
      - Stata 19 or newer
      - an installed fereg package
      - reghdfe 6.14.1 for the optional extended comparisons
      - a fresh Stata session after installing or replacing the native plugin

    The file is self-contained.  It simulates its own data,
    records one observation per test, continues after individual failures,
    and leaves the results frame as the current dataset.  Timings are
    end-to-end diagnostics on small data, not performance qualification.

    Edit only the settings immediately below for a larger or longer run.
*/

version 18.0
clear all
set more off
set linesize 255

/* ---------------------------- User settings ---------------------------- */
local scale             1          // Multiplies the broad panel sample size.
local repetitions       1          // Number of timed runs per command and test.
local seed              20260831   // Random-number seed for reproducible data.
local threads           1          // Processors allowed for fereg and Stata.
local strict            1          // Return r(459) if any test fails (0: report only).
local require_reghdfe   0          // Treat missing reghdfe as skips (1: fail the suite).
/* ----------------------------------------------------------------------- */

local original_processors = c(processors)
if real("`c(stata_version)'") < 19 {
    display as error "manual_validation.do requires Stata 19 or newer"
    exit 9
}
if `scale' < 1 | `scale' != floor(`scale') {
    display as error "scale must be a positive integer"
    exit 198
}
if `repetitions' < 1 | `repetitions' != floor(`repetitions') {
    display as error "repetitions must be a positive integer"
    exit 198
}
if `threads' < 1 | `threads' != floor(`threads') | `threads' > c(processors) {
    display as error "threads must be an integer between 1 and c(processors)"
    exit 198
}
if !inlist(`strict', 0, 1) | !inlist(`require_reghdfe', 0, 1) {
    display as error "strict and require_reghdfe must be zero or one"
    exit 198
}

capture findfile fereg.ado
if _rc {
    display as error "fereg is not installed or is not on the ado-path"
    exit 111
}
local fereg_path `"`r(fn)'"'

capture findfile reghdfe.ado
local have_reghdfe = (_rc == 0)
local reghdfe_path
if `have_reghdfe' local reghdfe_path `"`r(fn)'"'

display as text "fereg referee validation suite"
display as text "  Stata:       `c(stata_version)' `c(flavor)'"
display as text "  OS/machine:  `c(os)' / `c(machine_type)'"
display as text "  processors:  `c(processors)' configured; `threads' used"
display as text "  scale:       `scale'"
display as text "  repetitions: `repetitions'"
display as text `"  fereg:       `fereg_path'"'
which fereg
capture noisily fereg, selftest threads(`threads')
local selftest_rc = _rc
if `selftest_rc' {
    display as error "fereg selftest failed with r(`selftest_rc')"
    exit `selftest_rc'
}
local fereg_plugin_path `"`r(plugin_file)'"'
display as text `"  native plugin: `fereg_plugin_path'"'
display as text "  plugin note: restart Stata after replacing a native plugin; clear all does not unload it"
if `have_reghdfe' {
    display as text `"  reghdfe:     `reghdfe_path'"'
    which reghdfe
}
else display as text "  reghdfe:     unavailable; extended cases will be skipped"

/* Matrix comparison returns max absolute error, max tolerance-scaled error,
   and a pass indicator.  The registered rule is
       abs(a-b) <= atol + rtol*max(abs(a),abs(b))
   element by element. */
mata:
real rowvector __frg_manual_matrix_distance(
    string scalar left_name,
    string scalar right_name,
    real scalar atol,
    real scalar rtol)
{
    real matrix left, right, scale, difference, limit

    left = st_matrix(left_name)
    right = st_matrix(right_name)
    difference = abs(left :- right)
    scale = (abs(left) :>= abs(right)) :* abs(left) :+
        (abs(left) :< abs(right)) :* abs(right)
    limit = atol :+ rtol :* scale
    return((max(difference), max(difference :/ limit), all(difference :<= limit)))
}
end

frame create __frg_manual_results                                     ///
    str40 test_id str20 category str12 comparator str10 status         ///
    byte failed int rc_fereg int rc_alt                                ///
    int rc_fereg_timing int rc_alt_timing double n_estimation          ///
    int n_checks int n_failed long sample_mismatch                     ///
    double b_max_abs double b_scaled_error                             ///
    double V_max_abs double V_scaled_error                             ///
    double scalar_max_abs double rss_relative_error                    ///
    double fereg_seconds double alt_seconds double time_ratio          ///
    str120 failed_checks str244 unchecked_common_e                     ///
    strL fereg_command strL comparator_command

frame __frg_manual_results: label variable test_id             "Test identifier"
frame __frg_manual_results: label variable category            "Test category"
frame __frg_manual_results: label variable comparator          "Reference estimator"
frame __frg_manual_results: label variable status              "Test status: pass, fail, or skip"
frame __frg_manual_results: label variable failed              "Indicator that the test failed"
frame __frg_manual_results: label variable rc_fereg            "Return code from fereg"
frame __frg_manual_results: label variable rc_alt              "Return code from reference estimator"
frame __frg_manual_results: label variable rc_fereg_timing     "Return code from the timed fereg run"
frame __frg_manual_results: label variable rc_alt_timing       "Return code from the timed comparator run"
frame __frg_manual_results: label variable n_estimation        "Observations in the fereg estimation sample"
frame __frg_manual_results: label variable n_checks            "Number of comparisons performed"
frame __frg_manual_results: label variable n_failed            "Number of failed comparisons"
frame __frg_manual_results: label variable sample_mismatch     "Observations with different sample membership"
frame __frg_manual_results: label variable b_max_abs           "Maximum absolute coefficient difference"
frame __frg_manual_results: label variable b_scaled_error      "Maximum scaled coefficient error"
frame __frg_manual_results: label variable V_max_abs           "Maximum absolute covariance difference"
frame __frg_manual_results: label variable V_scaled_error      "Maximum scaled covariance error"
frame __frg_manual_results: label variable scalar_max_abs      "Maximum absolute e-class scalar difference"
frame __frg_manual_results: label variable rss_relative_error  "Relative residual sum-of-squares error"
frame __frg_manual_results: label variable fereg_seconds       "Elapsed fereg time in seconds"
frame __frg_manual_results: label variable alt_seconds         "Elapsed reference-estimator time in seconds"
frame __frg_manual_results: label variable time_ratio          "fereg time divided by reference-estimator time"
frame __frg_manual_results: label variable failed_checks       "Names of failed comparisons"
frame __frg_manual_results: label variable unchecked_common_e  "Common e() results not compared"
frame __frg_manual_results: label variable fereg_command       "Full fereg command used in the test"
frame __frg_manual_results: label variable comparator_command  "Full comparator command used in the test"

global FRM_HAVE_RH `have_reghdfe'
global FRM_REPS `repetitions'

capture program drop __frg_manual_compare
program define __frg_manual_compare
    version 18.0

    local test_id `"$FRM_TEST"'
    local category `"$FRM_CAT"'
    local comparator `"$FRM_ALT"'
    local fereg_command `"$FRM_FCMD"'
    local comparator_command `"$FRM_ACMD"'
    local needs_reghdfe = real("$FRM_NEED_RH")

    if `needs_reghdfe' & !$FRM_HAVE_RH {
        frame post __frg_manual_results                                ///
            (`"`test_id'"') (`"`category'"') (`"`comparator'"')       ///
            ("skip") (0) (.) (.) (.) (.) (.) (0) (0) (.)             ///
            (.) (.) (.) (.) (.) (.) (.) (.) (.)                      ///
            ("dependency_missing") ("")                              ///
            (`"`fereg_command'"') (`"`comparator_command'"')
        display as text "SKIP  `test_id' (reghdfe unavailable)"
        exit
    }

    local repetitions = real("$FRM_REPS")
    local timing_f_rc 0
    local timing_a_rc 0
    timer clear 91
    timer clear 92
    forvalues repetition = 1/`repetitions' {
        local candidate_first = mod(`repetition' + real("$FRM_INDEX"), 2)
        if `candidate_first' {
            timer on 91
            capture quietly $FRM_FCMD
            local run_rc = _rc
            timer off 91
            if `run_rc' & !`timing_f_rc' local timing_f_rc = `run_rc'

            timer on 92
            capture quietly $FRM_ACMD
            local run_rc = _rc
            timer off 92
            if `run_rc' & !`timing_a_rc' local timing_a_rc = `run_rc'
        }
        else {
            timer on 92
            capture quietly $FRM_ACMD
            local run_rc = _rc
            timer off 92
            if `run_rc' & !`timing_a_rc' local timing_a_rc = `run_rc'

            timer on 91
            capture quietly $FRM_FCMD
            local run_rc = _rc
            timer off 91
            if `run_rc' & !`timing_f_rc' local timing_f_rc = `run_rc'
        }
    }
    quietly timer list
    local fereg_seconds = r(t91) / `repetitions'
    local alt_seconds = r(t92) / `repetitions'
    timer clear 91
    timer clear 92

    local exact_count : word count $FRM_EXACT
    local numeric_count : word count $FRM_NUMERIC
    tempname f_exact f_exact_has f_numeric f_numeric_has
    tempname a_exact a_exact_has a_numeric a_numeric_has
    matrix `f_exact' = J(1, `exact_count', .)
    matrix `f_exact_has' = J(1, `exact_count', 0)
    matrix `f_numeric' = J(1, `numeric_count', .)
    matrix `f_numeric_has' = J(1, `numeric_count', 0)
    matrix `a_exact' = J(1, `exact_count', .)
    matrix `a_exact_has' = J(1, `exact_count', 0)
    matrix `a_numeric' = J(1, `numeric_count', .)
    matrix `a_numeric_has' = J(1, `numeric_count', 0)

    tempvar f_sample a_sample
    tempname f_b f_V a_b a_V
    local f_N .
    local f_scalar_names
    local f_depvar
    local f_vce
    local f_wtype
    local f_wexp

    capture quietly $FRM_FCMD
    local f_rc = _rc
    if !`f_rc' {
        generate byte `f_sample' = e(sample)
        label variable `f_sample' "fereg estimation-sample indicator"
        matrix `f_b' = e(b)
        matrix `f_V' = e(V)
        capture local f_N = e(N)
        local f_scalar_names : e(scalars)
        local f_depvar `"`e(depvar)'"'
        local f_vce `"`e(vce)'"'
        local f_wtype `"`e(wtype)'"'
        local f_wexp `"`e(wexp)'"'

        forvalues index = 1/`exact_count' {
            local scalar_name : word `index' of $FRM_EXACT
            local scalar_present : list scalar_name in f_scalar_names
            if `scalar_present' {
                matrix `f_exact'[1,`index'] = e(`scalar_name')
                matrix `f_exact_has'[1,`index'] = 1
            }
        }
        forvalues index = 1/`numeric_count' {
            local scalar_name : word `index' of $FRM_NUMERIC
            local scalar_present : list scalar_name in f_scalar_names
            if `scalar_present' {
                matrix `f_numeric'[1,`index'] = e(`scalar_name')
                matrix `f_numeric_has'[1,`index'] = 1
            }
        }
    }

    capture quietly $FRM_ACMD
    local a_rc = _rc
    local a_scalar_names
    local a_depvar
    local a_vce
    local a_wtype
    local a_wexp
    if !`a_rc' {
        generate byte `a_sample' = e(sample)
        label variable `a_sample' "Reference estimation-sample indicator"
        matrix `a_b' = e(b)
        matrix `a_V' = e(V)
        local a_scalar_names : e(scalars)
        local a_depvar `"`e(depvar)'"'
        local a_vce `"`e(vce)'"'
        local a_wtype `"`e(wtype)'"'
        local a_wexp `"`e(wexp)'"'

        forvalues index = 1/`exact_count' {
            local scalar_name : word `index' of $FRM_EXACT
            local scalar_present : list scalar_name in a_scalar_names
            if `scalar_present' {
                matrix `a_exact'[1,`index'] = e(`scalar_name')
                matrix `a_exact_has'[1,`index'] = 1
            }
        }
        forvalues index = 1/`numeric_count' {
            local scalar_name : word `index' of $FRM_NUMERIC
            local scalar_present : list scalar_name in a_scalar_names
            if `scalar_present' {
                matrix `a_numeric'[1,`index'] = e(`scalar_name')
                matrix `a_numeric_has'[1,`index'] = 1
            }
        }
    }

    local n_checks 2
    local n_failed 0
    local failed_checks
    local sample_mismatch .
    local b_max_abs .
    local b_scaled_error .
    local V_max_abs .
    local V_scaled_error .
    local scalar_max_abs .
    local rss_relative_error .

    if `f_rc' {
        local n_failed = `n_failed' + 1
        local failed_checks "`failed_checks' fereg_rc"
    }
    if `a_rc' {
        local n_failed = `n_failed' + 1
        local failed_checks "`failed_checks' alt_rc"
    }
    if `timing_f_rc' {
        local n_checks = `n_checks' + 1
        local n_failed = `n_failed' + 1
        local failed_checks "`failed_checks' fereg_timing_rc"
    }
    if `timing_a_rc' {
        local n_checks = `n_checks' + 1
        local n_failed = `n_failed' + 1
        local failed_checks "`failed_checks' alt_timing_rc"
    }

    if !`f_rc' & !`a_rc' {
        local n_checks = `n_checks' + 1
        quietly count if `f_sample' != `a_sample'
        local sample_mismatch = r(N)
        if `sample_mismatch' {
            local n_failed = `n_failed' + 1
            local failed_checks "`failed_checks' sample"
        }

        local common_coefs `"$FRM_COEFS"'
        tempname f_b_compare a_b_compare f_V_compare a_V_compare
        local matrices_ready 1
        if `"`common_coefs'"' == "" {
            matrix `f_b_compare' = `f_b'
            matrix `a_b_compare' = `a_b'
            matrix `f_V_compare' = `f_V'
            matrix `a_V_compare' = `a_V'

            local f_b_names : colfullnames `f_b_compare'
            local a_b_names : colfullnames `a_b_compare'
            local f_V_rows : rowfullnames `f_V_compare'
            local a_V_rows : rowfullnames `a_V_compare'
            local f_V_cols : colfullnames `f_V_compare'
            local a_V_cols : colfullnames `a_V_compare'

            local n_checks = `n_checks' + 2
            if rowsof(`f_b_compare') != rowsof(`a_b_compare') |          ///
               colsof(`f_b_compare') != colsof(`a_b_compare') |        ///
               `"`f_b_names'"' != `"`a_b_names'"' {
                local matrices_ready 0
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' b_axis"
            }
            if rowsof(`f_V_compare') != rowsof(`a_V_compare') |          ///
               colsof(`f_V_compare') != colsof(`a_V_compare') |        ///
               `"`f_V_rows'"' != `"`a_V_rows'"' |                    ///
               `"`f_V_cols'"' != `"`a_V_cols'"' {
                local matrices_ready 0
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' V_axis"
            }
        }
        else {
            local coefficient_count : word count `common_coefs'
            matrix `f_b_compare' = J(1, `coefficient_count', .)
            matrix `a_b_compare' = J(1, `coefficient_count', .)
            matrix `f_V_compare' = J(`coefficient_count', `coefficient_count', .)
            matrix `a_V_compare' = J(`coefficient_count', `coefficient_count', .)
            local n_checks = `n_checks' + 1

            forvalues column = 1/`coefficient_count' {
                local coefficient : word `column' of `common_coefs'
                local f_column = colnumb(`f_b', `"`coefficient'"')
                local a_column = colnumb(`a_b', `"`coefficient'"')
                if missing(`f_column') | missing(`a_column') {
                    local matrices_ready 0
                }
                else {
                    matrix `f_b_compare'[1,`column'] = `f_b'[1,`f_column']
                    matrix `a_b_compare'[1,`column'] = `a_b'[1,`a_column']
                    forvalues row = 1/`coefficient_count' {
                        local row_coefficient : word `row' of `common_coefs'
                        local f_row = colnumb(`f_b', `"`row_coefficient'"')
                        local a_row = colnumb(`a_b', `"`row_coefficient'"')
                        if missing(`f_row') | missing(`a_row') {
                            local matrices_ready 0
                        }
                        else {
                            matrix `f_V_compare'[`row',`column'] =       ///
                                `f_V'[`f_row',`f_column']
                            matrix `a_V_compare'[`row',`column'] =       ///
                                `a_V'[`a_row',`a_column']
                        }
                    }
                }
            }
            if !`matrices_ready' {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' common_coefficients"
            }
        }

        if `matrices_ready' {
            tempname b_metrics V_metrics
            mata: st_matrix("`b_metrics'", __frg_manual_matrix_distance( ///
                "`f_b_compare'", "`a_b_compare'", 1e-10, 1e-7))
            mata: st_matrix("`V_metrics'", __frg_manual_matrix_distance( ///
                "`f_V_compare'", "`a_V_compare'", 1e-10, 1e-6))
            local b_max_abs = `b_metrics'[1,1]
            local b_scaled_error = `b_metrics'[1,2]
            local V_max_abs = `V_metrics'[1,1]
            local V_scaled_error = `V_metrics'[1,2]
            local n_checks = `n_checks' + 2
            if `b_metrics'[1,3] != 1 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' b"
            }
            if `V_metrics'[1,3] != 1 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' V"
            }
        }

        forvalues index = 1/`exact_count' {
            local scalar_name : word `index' of $FRM_EXACT
            local n_checks = `n_checks' + 1
            if `f_exact_has'[1,`index'] != 1 | `a_exact_has'[1,`index'] != 1 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' `scalar_name'_defined"
            }
            else if `f_exact'[1,`index'] != `a_exact'[1,`index'] {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' `scalar_name'"
            }
        }

        forvalues index = 1/`numeric_count' {
            local scalar_name : word `index' of $FRM_NUMERIC
            local n_checks = `n_checks' + 1
            if `f_numeric_has'[1,`index'] != 1 | `a_numeric_has'[1,`index'] != 1 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' `scalar_name'_defined"
            }
            else {
                local f_value = `f_numeric'[1,`index']
                local a_value = `a_numeric'[1,`index']
                if missing(`f_value') | missing(`a_value') {
                    if !missing(`f_value') | !missing(`a_value') {
                        local n_failed = `n_failed' + 1
                        local failed_checks "`failed_checks' `scalar_name'"
                    }
                }
                else {
                    local absolute_error = abs(`f_value' - `a_value')
                    local scalar_scale = max(abs(`f_value'), abs(`a_value'))
                    local scalar_limit = 1e-10 + 1e-7*`scalar_scale'
                    if missing(`scalar_max_abs') |                         ///
                       `absolute_error' > `scalar_max_abs'                  ///
                        local scalar_max_abs = `absolute_error'
                    if `"`scalar_name'"' == "rss"                         ///
                        local rss_relative_error =                          ///
                            `absolute_error' / max(abs(`f_value'), 1)
                    if `absolute_error' > `scalar_limit' {
                        local n_failed = `n_failed' + 1
                        local failed_checks "`failed_checks' `scalar_name'"
                    }
                }
            }
        }

        local n_checks = `n_checks' + 2
        if `"`f_depvar'"' != `"`a_depvar'"' {
            local n_failed = `n_failed' + 1
            local failed_checks "`failed_checks' depvar"
        }
        if `"`f_vce'"' != `"`a_vce'"' {
            local n_failed = `n_failed' + 1
            local failed_checks "`failed_checks' vce"
        }
        if real("$FRM_WEIGHT") {
            local n_checks = `n_checks' + 2
            if `"`f_wtype'"' != `"`a_wtype'"' {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' wtype"
            }
            if `"`f_wexp'"' != `"`a_wexp'"' {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' wexp"
            }
        }
    }

    local common_scalars : list f_scalar_names & a_scalar_names
    local classified `"$FRM_EXACT $FRM_NUMERIC"'
    local unchecked : list common_scalars - classified
    local noncomparable `"$FRM_NONCOMP"'
    local unchecked : list unchecked - noncomparable
    local unchecked_without_fereg
    foreach scalar_name of local unchecked {
        if substr(`"`scalar_name'"', 1, 6) != "fereg_"               ///
            local unchecked_without_fereg                            ///
                `"`unchecked_without_fereg' `scalar_name'"'
    }
    local unchecked `"`unchecked_without_fereg'"'
    local unchecked = substr(strtrim(`"`unchecked'"'), 1, 244)
    local failed_checks = substr(strtrim(`"`failed_checks'"'), 1, 120)

    local failed = (`n_failed' > 0)
    local status = cond(`failed', "fail", "pass")
    local time_ratio .
    if `alt_seconds' > 0 local time_ratio = `fereg_seconds' / `alt_seconds'

    frame post __frg_manual_results                                    ///
        (`"`test_id'"') (`"`category'"') (`"`comparator'"')           ///
        (`"`status'"') (`failed') (`f_rc') (`a_rc')                   ///
        (`timing_f_rc') (`timing_a_rc') (`f_N')                        ///
        (`n_checks') (`n_failed') (`sample_mismatch')                  ///
        (`b_max_abs') (`b_scaled_error') (`V_max_abs')                 ///
        (`V_scaled_error') (`scalar_max_abs') (`rss_relative_error')   ///
        (`fereg_seconds') (`alt_seconds') (`time_ratio')               ///
        (`"`failed_checks'"') (`"`unchecked'"')                     ///
        (`"`fereg_command'"') (`"`comparator_command'"')

    if `failed' display as error                                      ///
        "FAIL  `test_id': `failed_checks' [fereg r(`f_rc'); comparator r(`a_rc'); timed fereg r(`timing_f_rc'); timed comparator r(`timing_a_rc')]"
    else display as result "PASS  `test_id'"
end

capture program drop __frg_manual_expected_error
program define __frg_manual_expected_error
    version 18.0

    local test_id `"$FRM_TEST"'
    local comparator `"$FRM_ALT"'
    local fereg_command `"$FRM_FCMD"'
    local comparator_command `"$FRM_ACMD"'
    if real("$FRM_NEED_RH") & !$FRM_HAVE_RH {
        frame post __frg_manual_results                                ///
            (`"`test_id'"') ("failure") (`"`comparator'"') ("skip") ///
            (0) (.) (.) (.) (.) (.) (0) (0) (.)                      ///
            (.) (.) (.) (.) (.) (.) (.) (.) (.)                      ///
            ("dependency_missing") ("")                              ///
            (`"`fereg_command'"') (`"`comparator_command'"')
        display as text "SKIP  `test_id' (reghdfe unavailable)"
        exit
    }

    timer clear 91
    timer on 91
    capture quietly $FRM_FCMD
    local f_rc = _rc
    timer off 91
    quietly timer list 91
    local fereg_seconds = r(t91)
    timer clear 91

    timer clear 92
    timer on 92
    capture quietly $FRM_ACMD
    local a_rc = _rc
    timer off 92
    quietly timer list 92
    local alt_seconds = r(t92)
    timer clear 92

    local expected_f = real("$FRM_EXPECT_F")
    local expected_a = real("$FRM_EXPECT_A")
    local n_failed 0
    local failed_checks
    if `f_rc' != `expected_f' {
        local n_failed = `n_failed' + 1
        local failed_checks "fereg_rc"
    }
    if `a_rc' != `expected_a' {
        local n_failed = `n_failed' + 1
        local failed_checks "`failed_checks' alt_rc"
    }
    local failed = (`n_failed' > 0)
    local status = cond(`failed', "fail", "pass")
    local time_ratio .
    if `alt_seconds' > 0 local time_ratio = `fereg_seconds' / `alt_seconds'
    local failed_checks = strtrim(`"`failed_checks'"')

    frame post __frg_manual_results                                    ///
        (`"`test_id'"') ("failure") (`"`comparator'"')               ///
        (`"`status'"') (`failed') (`f_rc') (`a_rc')                   ///
        (`f_rc') (`a_rc') (.) (2)                                     ///
        (`n_failed') (.) (.) (.) (.) (.) (.) (.)                      ///
        (`fereg_seconds') (`alt_seconds') (`time_ratio')               ///
        (`"`failed_checks'"') ("")                                  ///
        (`"`fereg_command'"') (`"`comparator_command'"')

    if `failed' display as error                                      ///
        "FAIL  `test_id': `failed_checks' [fereg r(`f_rc'); comparator r(`a_rc')]"
    else display as result "PASS  `test_id'"
end

capture program drop __frg_manual_saved_outputs
program define __frg_manual_saved_outputs
    version 18.0

    local fereg_command                                                ///
        "fereg y c.x1 c.x2, absorb(__frg_manual_fe_worker=worker __frg_manual_fe_firm=firm) residuals(__frg_manual_residual) threads($FRM_THREADS) tolerance(1e-10)"
    local comparator_command                                           ///
        "predict double __frg_manual_xb, xb; predict double __frg_manual_xbd, xbd; predict double __frg_manual_d, d; predict double __frg_manual_predict_residual, residuals"

    capture drop __frg_manual_fe_worker __frg_manual_fe_firm          ///
        __frg_manual_residual __frg_manual_xb __frg_manual_xbd        ///
        __frg_manual_d __frg_manual_predict_residual

    timer clear 91
    timer on 91
    capture quietly fereg y c.x1 c.x2,                                 ///
        absorb(__frg_manual_fe_worker=worker                           ///
               __frg_manual_fe_firm=firm)                             ///
        residuals(__frg_manual_residual) threads($FRM_THREADS)         ///
        tolerance(1e-10)
    local f_rc = _rc
    timer off 91
    quietly timer list 91
    local fereg_seconds = r(t91)
    timer clear 91

    local n_checks 1
    local n_failed 0
    local failed_checks
    local f_N .
    local reconstruction_max .
    if `f_rc' {
        local n_failed = 1
        local failed_checks "fereg_rc"
    }
    else {
        local f_N = e(N)
        tempvar estimation_sample difference
        generate byte `estimation_sample' = e(sample)
        label variable `estimation_sample' "fereg estimation-sample indicator"

        label variable __frg_manual_fe_worker "Saved worker fixed effect"
        label variable __frg_manual_fe_firm "Saved firm fixed effect"
        label variable __frg_manual_residual "Saved fereg residual"

        capture quietly predict double __frg_manual_xb, xb
        local predict_xb_rc = _rc
        if !`predict_xb_rc' label variable __frg_manual_xb "Predicted covariate component"
        capture quietly predict double __frg_manual_xbd, xbd
        local predict_xbd_rc = _rc
        if !`predict_xbd_rc' label variable __frg_manual_xbd "Predicted covariate and fixed-effect components"
        capture quietly predict double __frg_manual_d, d
        local predict_d_rc = _rc
        if !`predict_d_rc' label variable __frg_manual_d "Predicted fixed-effect component"
        capture quietly predict double __frg_manual_predict_residual, residuals
        local predict_residual_rc = _rc
        if !`predict_residual_rc' label variable __frg_manual_predict_residual "Predicted residual"
        foreach check in predict_xb predict_xbd predict_d predict_residual {
            local n_checks = `n_checks' + 1
            if ``check'_rc' {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' `check'"
            }
        }

        if !`predict_xb_rc' & !`predict_xbd_rc' & !`predict_d_rc' &   ///
           !`predict_residual_rc' {
            local n_checks = `n_checks' + 1
            quietly count if `estimation_sample' &                    ///
                missing(__frg_manual_fe_worker, __frg_manual_fe_firm, ///
                    __frg_manual_residual, __frg_manual_xb,           ///
                    __frg_manual_xbd, __frg_manual_d,                 ///
                    __frg_manual_predict_residual)
            if r(N) {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' output_missing"
            }

            generate double `difference' = abs(y - __frg_manual_xb - ///
                __frg_manual_d - __frg_manual_residual)               ///
                if `estimation_sample'
            label variable `difference' "Absolute postestimation identity difference"
            quietly summarize `difference', meanonly
            local reconstruction_max = r(max)
            local n_checks = `n_checks' + 1
            if `reconstruction_max' > 1e-9 + 1e-7*max(abs(r(max)), 1) {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' reconstruction"
            }

            replace `difference' = abs(__frg_manual_xbd -             ///
                __frg_manual_xb - __frg_manual_d) if `estimation_sample'
            quietly summarize `difference', meanonly
            local n_checks = `n_checks' + 1
            if r(max) > 1e-9 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' xbd_identity"
            }

            replace `difference' = abs(__frg_manual_d -               ///
                __frg_manual_fe_worker - __frg_manual_fe_firm)        ///
                if `estimation_sample'
            quietly summarize `difference', meanonly
            local n_checks = `n_checks' + 1
            if r(max) > 1e-9 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' absorbed_identity"
            }

            replace `difference' = abs(__frg_manual_residual -        ///
                __frg_manual_predict_residual) if `estimation_sample'
            quietly summarize `difference', meanonly
            local n_checks = `n_checks' + 1
            if r(max) > 1e-12 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' residual_predict"
            }
        }
    }

    local failed = (`n_failed' > 0)
    local status = cond(`failed', "fail", "pass")
    local failed_checks = substr(strtrim(`"`failed_checks'"'), 1, 120)
    frame post __frg_manual_results                                    ///
        ("saved_outputs") ("postestimation") ("internal")            ///
        (`"`status'"') (`failed') (`f_rc') (.) (`f_rc') (.) (`f_N')  ///
        (`n_checks') (`n_failed') (.) (.) (.) (.) (.)                 ///
        (`reconstruction_max') (.) (`fereg_seconds') (.) (.)          ///
        (`"`failed_checks'"') ("")                                  ///
        (`"`fereg_command'"') (`"`comparator_command'"')

    if `failed' display as error                                      ///
        "FAIL  saved_outputs: `failed_checks' [fereg r(`f_rc')]"
    else display as result "PASS  saved_outputs"

    capture drop __frg_manual_fe_worker __frg_manual_fe_firm          ///
        __frg_manual_residual __frg_manual_xb __frg_manual_xbd        ///
        __frg_manual_d __frg_manual_predict_residual
end

capture program drop __frg_manual_postestimation
program define __frg_manual_postestimation
    version 18.0

    local fereg_command                                                ///
        "fereg y c.x1 c.x2, threads($FRM_THREADS) tolerance(1e-10); test x1; estat summarize"
    local comparator_command                                           ///
        "regress y c.x1 c.x2; test x1; estat summarize"

    tempname f_test f_stats a_test a_stats test_metrics stats_metrics
    local n_checks 2
    local n_failed 0
    local failed_checks
    local f_N .

    timer clear 91
    timer on 91
    capture quietly fereg y c.x1 c.x2, threads($FRM_THREADS) tolerance(1e-10)
    local f_rc = _rc
    timer off 91
    quietly timer list 91
    local fereg_seconds = r(t91)
    timer clear 91
    if `f_rc' {
        local n_failed = `n_failed' + 1
        local failed_checks "fereg_rc"
    }
    else {
        local f_N = e(N)
        capture quietly test x1
        local f_test_rc = _rc
        if !`f_test_rc' matrix `f_test' = (r(F), r(df), r(df_r), r(p))
        capture quietly estat summarize
        local f_estat_rc = _rc
        if !`f_estat_rc' matrix `f_stats' = r(stats)
    }

    timer clear 92
    timer on 92
    capture quietly regress y c.x1 c.x2
    local a_rc = _rc
    timer off 92
    quietly timer list 92
    local alt_seconds = r(t92)
    timer clear 92
    if `a_rc' {
        local n_failed = `n_failed' + 1
        local failed_checks "`failed_checks' alt_rc"
    }
    else {
        capture quietly test x1
        local a_test_rc = _rc
        if !`a_test_rc' matrix `a_test' = (r(F), r(df), r(df_r), r(p))
        capture quietly estat summarize
        local a_estat_rc = _rc
        if !`a_estat_rc' matrix `a_stats' = r(stats)
    }

    if !`f_rc' & !`a_rc' {
        local n_checks = `n_checks' + 2
        if `f_test_rc' | `a_test_rc' {
            local n_failed = `n_failed' + 1
            local failed_checks "`failed_checks' test_rc"
        }
        else {
            mata: st_matrix("`test_metrics'", __frg_manual_matrix_distance( ///
                "`f_test'", "`a_test'", 1e-10, 1e-7))
            local n_checks = `n_checks' + 1
            if `test_metrics'[1,3] != 1 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' test_results"
            }
        }
        if `f_estat_rc' | `a_estat_rc' {
            local n_failed = `n_failed' + 1
            local failed_checks "`failed_checks' estat_rc"
        }
        else {
            mata: st_matrix("`stats_metrics'", __frg_manual_matrix_distance( ///
                "`f_stats'", "`a_stats'", 1e-10, 1e-7))
            local n_checks = `n_checks' + 1
            if `stats_metrics'[1,3] != 1 {
                local n_failed = `n_failed' + 1
                local failed_checks "`failed_checks' estat_results"
            }
        }
    }

    local failed = (`n_failed' > 0)
    local status = cond(`failed', "fail", "pass")
    local time_ratio .
    if `alt_seconds' > 0 local time_ratio = `fereg_seconds' / `alt_seconds'
    local failed_checks = substr(strtrim(`"`failed_checks'"'), 1, 120)
    local scalar_error .
    if !`f_rc' & !`a_rc' & !`f_test_rc' & !`a_test_rc'               ///
        local scalar_error = `test_metrics'[1,1]

    frame post __frg_manual_results                                    ///
        ("test_and_estat") ("postestimation") ("regress")            ///
        (`"`status'"') (`failed') (`f_rc') (`a_rc')                   ///
        (`f_rc') (`a_rc') (`f_N')                                     ///
        (`n_checks') (`n_failed') (.) (.) (.) (.) (.)                 ///
        (`scalar_error') (.) (`fereg_seconds') (`alt_seconds')        ///
        (`time_ratio') (`"`failed_checks'"') ("")                   ///
        (`"`fereg_command'"') (`"`comparator_command'"')

    if `failed' display as error                                      ///
        "FAIL  test_and_estat: `failed_checks' [fereg r(`f_rc'); comparator r(`a_rc')]"
    else display as result "PASS  test_and_estat"
end

/* -------------------------- Simulated base data ------------------------ */
frame create __frg_manual_data
frame change __frg_manual_data
set rng mt64
set seed `seed'
set processors `threads'

local workers = 200 * `scale'
local firms = 20 * `scale'
local periods = 5
local observations = `workers' * `periods'
local oracle_workers = 40

quietly set obs `workers'
generate long worker = _n
quietly expand `periods'
bysort worker: generate byte period = _n
generate int year = 2017 + period
generate double time = period - 1
generate long firm_static = mod(worker - 1, `firms') + 1
bysort worker (period): generate long firm = firm_static if _n == 1
bysort worker (period): replace firm =                           ///
    cond(mod(worker + 3*period, 7) == 0,                         ///
         mod(worker + 7919*period - 1, `firms') + 1, firm[_n-1]) ///
    if _n > 1

bysort worker: generate double worker_fe = rnormal(0, .4) if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm: generate double firm_fe = rnormal(0, .2) if _n == 1
bysort firm: replace firm_fe = firm_fe[1]
generate double x1 = rnormal()
generate double x2 = .35*x1 + rnormal()
generate byte industry = mod(firm - 1, 3) + 1
generate long cluster_a = mod(worker + period, 31*`scale') + 1
generate long cluster_b = mod(firm + 2*period, 17*`scale') + 1
generate double y = 1 + .6*x1 - .3*x2 + .02*time +             ///
    worker_fe + firm_fe + rnormal(0, .5)
generate double aw = .5 + mod(_n, 11)/10
generate long fw = 1 + mod(_n, 3)
generate double pw = .8 + mod(_n, 13)/20
generate double iw = 1 + mod(_n, 7)/10
generate double x_missing = x1
replace x_missing = . if mod(_n, 17) == 0
generate double aw_negative = aw
replace aw_negative = -1 in 1
generate double fw_fractional = fw
replace fw_fractional = 1.5 in 1
generate str18 firm_s = "firm_" + string(firm, "%08.0f")
sort worker period
generate long obs_id = _n

label variable worker        "Worker identifier"
label variable period        "Within-worker period index"
label variable year          "Calendar year"
label variable time          "Linear time index"
label variable firm_static   "Worker's initial firm identifier"
label variable firm          "Current firm identifier"
label variable worker_fe     "Simulated worker fixed effect"
label variable firm_fe       "Simulated firm fixed effect"
label variable x1            "First simulated regressor"
label variable x2            "Second simulated regressor"
label variable industry      "Simulated industry identifier"
label variable cluster_a     "First clustering identifier"
label variable cluster_b     "Second clustering identifier"
label variable y             "Simulated outcome"
label variable aw            "Positive analytic weight"
label variable fw            "Positive integer frequency weight"
label variable pw            "Positive probability weight"
label variable iw            "Importance weight used for rejection test"
label variable x_missing     "First regressor with systematic missing values"
label variable aw_negative   "Analytic weight with one invalid negative value"
label variable fw_fractional "Frequency weight with one invalid fractional value"
label variable firm_s        "String-valued firm identifier"
label variable obs_id        "Observation identifier"

xtset worker period
assert _N == `observations'

global FRM_THREADS `threads'

/* Warm the command-loading paths once.  These runs are not recorded. */
capture quietly fereg y c.x1 c.x2, threads(`threads') tolerance(1e-10)
local warmup_rc = _rc
if `warmup_rc' {
    display as error "fereg warm-up failed with r(`warmup_rc')"
    exit `warmup_rc'
}
capture quietly regress y c.x1 c.x2
capture quietly areg y c.x1 c.x2, absorb(worker)
if `have_reghdfe' capture quietly reghdfe y c.x1 c.x2, absorb(worker firm) ///
    dof(pairwise) tolerance(1e-10) iterate(16000)

/* ------------------------ Comparator test matrix ----------------------- */
local cases no_fe factor_rhs one_fe_areg two_fe_areg explicit_dummy     ///
    two_fe_reghdfe robust_reghdfe cluster_twoway aweight fweight       ///
    pweight string_fe interacted_fe absorbed_slope time_series         ///
    missing_if no_regressors noconstant solver_lsmr memory_modes

local test_index 0
foreach case of local cases {
    local test_index = `test_index' + 1
    global FRM_INDEX `test_index'
    global FRM_TEST "`case'"
    global FRM_CAT "model"
    global FRM_ALT "regress"
    global FRM_NEED_RH 0
    global FRM_WEIGHT 0
    global FRM_COEFS ""
    global FRM_EXACT "N df_m df_r"
    global FRM_NUMERIC "rss mss r2 r2_a rmse F ll ll_0"
    global FRM_NONCOMP                              ///
        "rank df_a df_a_initial df_a_redundant df_a_nested num_singletons drop_singletons N_full sumweights ic converged report_constant N_hdfe N_hdfe_extended"

    if "`case'" == "no_fe" {
        global FRM_FCMD                                      ///
            "fereg y c.x1 c.x2, threads(`threads') tolerance(1e-10)"
        global FRM_ACMD "regress y c.x1 c.x2"
    }
    else if "`case'" == "factor_rhs" {
        global FRM_FCMD                                      ///
            "fereg y c.x1 i.year, threads(`threads') tolerance(1e-10)"
        global FRM_ACMD "regress y c.x1 i.year"
    }
    else if "`case'" == "one_fe_areg" {
        global FRM_CAT "fixed_effects"
        global FRM_ALT "areg"
        global FRM_EXACT "N"
        global FRM_NUMERIC "rss"
        /* Stata 19 areg uses different absorbed-DoF and derived-fit
           reporting conventions.  Coefficients, VCE, sample, N, and RSS
           are the registered comparable surface for this arm. */
        global FRM_NONCOMP                                           ///
            "rank df_a df_a_initial df_a_redundant df_a_nested num_singletons drop_singletons N_full sumweights ic converged report_constant N_hdfe N_hdfe_extended df_r tss mss r2 r2_a rmse F ll ll_0 df_m"
        global FRM_FCMD                                      ///
            "fereg y c.x1 c.x2, absorb(worker) dof(pairwise) threads(`threads') tolerance(1e-10)"
        global FRM_ACMD "areg y c.x1 c.x2, absorb(worker)"
    }
    else if "`case'" == "two_fe_areg" {
        global FRM_CAT "fixed_effects"
        global FRM_ALT "areg"
        global FRM_EXACT "N"
        global FRM_NUMERIC "rss"
        global FRM_NONCOMP                                           ///
            "rank df_a df_a_initial df_a_redundant df_a_nested num_singletons drop_singletons N_full sumweights ic converged report_constant N_hdfe N_hdfe_extended df_r tss mss r2 r2_a rmse F ll ll_0 df_m"
        global FRM_FCMD                                      ///
            "fereg y c.x1 c.x2, absorb(worker firm) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
        global FRM_ACMD                                      ///
            "areg y c.x1 c.x2, absorb(worker firm) dfabsorb noabsorbtest tolerance(1e-10) iterate(16000) nolog"
    }
    else if "`case'" == "explicit_dummy" {
        global FRM_CAT "oracle"
        global FRM_ALT "regress"
        global FRM_COEFS "x1 x2"
        global FRM_EXACT "N df_r"
        global FRM_NUMERIC "rss mss r2 r2_a rmse ll"
        global FRM_NONCOMP                                           ///
            "rank df_a df_a_initial df_a_redundant df_a_nested num_singletons drop_singletons N_full sumweights ic converged report_constant N_hdfe N_hdfe_extended F df_m ll_0"
        global FRM_FCMD                                      ///
            "fereg y c.x1 c.x2 if worker <= `oracle_workers', absorb(worker firm_static) dof(pairwise) threads(`threads') tolerance(1e-10)"
        global FRM_ACMD                                      ///
            "regress y c.x1 c.x2 i.worker i.firm_static if worker <= `oracle_workers'"
    }
    else if inlist("`case'", "two_fe_reghdfe", "robust_reghdfe",     ///
                   "cluster_twoway", "aweight", "fweight", "pweight") | ///
            inlist("`case'", "string_fe", "interacted_fe",          ///
                   "absorbed_slope", "time_series", "missing_if",  ///
                   "no_regressors", "solver_lsmr") {
        global FRM_ALT "reghdfe"
        global FRM_NEED_RH 1
        global FRM_EXACT                             ///
            "N df_m df_r df_a num_singletons"
        global FRM_NUMERIC                           ///
            "rss tss tss_within mss r2 r2_a r2_within r2_a_within rmse F ll ll_0 sumweights"

        if "`case'" == "two_fe_reghdfe" {
            global FRM_CAT "fixed_effects"
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2, absorb(worker firm) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2, absorb(worker firm) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "robust_reghdfe" {
            global FRM_CAT "inference"
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2, absorb(worker firm) vce(robust) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2, absorb(worker firm) vce(robust) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "cluster_twoway" {
            global FRM_CAT "inference"
            global FRM_EXACT                         ///
                "N df_m df_r df_a num_singletons N_clust N_clust1 N_clust2 N_clustervars"
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2, absorb(worker firm) vce(cluster cluster_a cluster_b) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2, absorb(worker firm) vce(cluster cluster_a cluster_b) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "aweight" {
            global FRM_CAT "weights"
            global FRM_WEIGHT 1
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2 [aw=aw], absorb(worker firm) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2 [aw=aw], absorb(worker firm) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "fweight" {
            global FRM_CAT "weights"
            global FRM_WEIGHT 1
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2 [fw=fw], absorb(worker firm) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2 [fw=fw], absorb(worker firm) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "pweight" {
            global FRM_CAT "weights"
            global FRM_WEIGHT 1
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2 [pw=pw], absorb(worker firm) vce(robust) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2 [pw=pw], absorb(worker firm) vce(robust) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "string_fe" {
            global FRM_CAT "fixed_effects"
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2, absorb(firm_s year) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2, absorb(firm_s year) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "interacted_fe" {
            global FRM_CAT "fixed_effects"
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2, absorb(worker firm#year) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2, absorb(worker firm#year) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "absorbed_slope" {
            global FRM_CAT "fixed_effects"
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2, absorb(firm##c.time worker) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2, absorb(firm##c.time worker) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "time_series" {
            global FRM_CAT "syntax"
            global FRM_FCMD                                  ///
                "fereg y L.x1, absorb(worker) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y L.x1, absorb(worker) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "missing_if" {
            global FRM_CAT "sample"
            global FRM_FCMD                                  ///
                "fereg y c.x_missing if mod(worker,4) != 0, absorb(worker firm) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x_missing if mod(worker,4) != 0, absorb(worker firm) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "no_regressors" {
            global FRM_CAT "model"
            global FRM_FCMD                                  ///
                "fereg y, absorb(worker firm) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y, absorb(worker firm) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
        else if "`case'" == "solver_lsmr" {
            global FRM_CAT "execution"
            global FRM_FCMD                                  ///
                "fereg y c.x1 c.x2, absorb(worker firm) solver(lsmr) preconditioner(schwarz) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
            global FRM_ACMD                                  ///
                "reghdfe y c.x1 c.x2, absorb(worker firm) dof(pairwise) tolerance(1e-10) iterate(16000)"
        }
    }
    else if "`case'" == "noconstant" {
        global FRM_CAT "model"
        /* fereg's noconstant suppresses the normalized reported intercept;
           the finite slopes therefore compare to the slope submatrix from
           the corresponding intercept model. */
        global FRM_COEFS "x1 x2"
        global FRM_EXACT "N df_m df_r"
        global FRM_NUMERIC "rss mss r2 r2_a rmse F ll ll_0"
        global FRM_FCMD                                      ///
            "fereg y c.x1 c.x2, noconstant threads(`threads') tolerance(1e-10)"
        global FRM_ACMD "regress y c.x1 c.x2"
    }
    else if "`case'" == "memory_modes" {
        global FRM_CAT "execution"
        global FRM_ALT "fereg_fast"
        global FRM_EXACT                             ///
            "N df_m df_r df_a num_singletons"
        global FRM_NUMERIC                           ///
            "rss tss tss_within mss r2 r2_a r2_within r2_a_within rmse F ll ll_0 sumweights"
        global FRM_FCMD                                      ///
            "fereg y c.x1 c.x2, absorb(worker) memory(low) poolsize(1) dof(pairwise) threads(`threads') tolerance(1e-8) iterate(16000)"
        global FRM_ACMD                                      ///
            "fereg y c.x1 c.x2, absorb(worker) memory(fast) dof(pairwise) threads(`threads') tolerance(1e-8) iterate(16000)"
    }

    __frg_manual_compare
}

/* A fixed micro-design exercises singleton pruning without being affected by
   the scale setting used for the broad panel cases. */
preserve
clear
set obs 12
generate long obs_id = 100 + _n
generate double y = real(word(                                      ///
    "3.20 4.05 4.60 5.25 5.90 6.35 7.10 7.85 8.10 8.75 9.30 10.05", _n))
generate double x = real(word(                                      ///
    "-1.40 -0.90 -0.45 0.05 0.40 0.85 1.15 1.65 -1.10 -0.25 0.70 1.90", _n))
generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 4 4", _n))
generate int firm = real(word("10 10 20 20 10 10 20 20 10 30 30 40", _n))
generate int year = real(word(                                      ///
    "2020 2020 2021 2021 2020 2020 2021 2021 2020 2022 2022 2023", _n))

label variable obs_id "Micro-design observation identifier"
label variable y      "Micro-design outcome"
label variable x      "Micro-design regressor"
label variable worker "Micro-design worker identifier"
label variable firm   "Micro-design firm identifier"
label variable year   "Micro-design calendar year"

local test_index = `test_index' + 1
global FRM_INDEX `test_index'
global FRM_TEST "singleton_cascade"
global FRM_CAT "sample"
global FRM_ALT "reghdfe"
global FRM_NEED_RH 1
global FRM_WEIGHT 0
global FRM_COEFS ""
global FRM_EXACT "N df_m df_r df_a num_singletons"
global FRM_NUMERIC                            ///
    "rss tss tss_within mss r2 r2_a r2_within r2_a_within rmse F ll ll_0 sumweights"
global FRM_NONCOMP                           ///
    "rank df_a_initial df_a_redundant df_a_nested drop_singletons N_full ic converged report_constant N_hdfe N_hdfe_extended"
global FRM_FCMD                                      ///
    "fereg y c.x, absorb(worker firm#year) dof(pairwise) threads(`threads') tolerance(1e-10) iterate(16000)"
global FRM_ACMD                                      ///
    "reghdfe y c.x, absorb(worker firm#year) dof(pairwise) tolerance(1e-10) iterate(16000)"
__frg_manual_compare
restore

/* Saved-output and public postestimation identities. */
__frg_manual_saved_outputs
__frg_manual_postestimation

/* Expected-error parity with the pinned reghdfe behavior. */
local error_cases negative_aweight fractional_fweight iweight_rejected
foreach case of local error_cases {
    global FRM_TEST "`case'"
    global FRM_ALT "reghdfe"
    global FRM_NEED_RH 1
    if "`case'" == "negative_aweight" {
        global FRM_FCMD                                     ///
            "fereg y c.x1 [aw=aw_negative], absorb(worker)"
        global FRM_ACMD                                     ///
            "reghdfe y c.x1 [aw=aw_negative], absorb(worker)"
        global FRM_EXPECT_F 402
        global FRM_EXPECT_A 402
    }
    else if "`case'" == "fractional_fweight" {
        global FRM_FCMD                                     ///
            "fereg y c.x1 [fw=fw_fractional], absorb(worker)"
        global FRM_ACMD                                     ///
            "reghdfe y c.x1 [fw=fw_fractional], absorb(worker)"
        global FRM_EXPECT_F 401
        global FRM_EXPECT_A 401
    }
    else if "`case'" == "iweight_rejected" {
        global FRM_FCMD "fereg y c.x1 [iw=iw], absorb(worker)"
        global FRM_ACMD "reghdfe y c.x1 [iw=iw], absorb(worker)"
        global FRM_EXPECT_F 101
        global FRM_EXPECT_A 101
    }
    __frg_manual_expected_error
}

/* ------------------------------ Summary ------------------------------- */
frame change __frg_manual_results
frame drop __frg_manual_data
sort status category test_id
format fereg_seconds alt_seconds %10.4f
format time_ratio %9.3f
format b_max_abs V_max_abs scalar_max_abs rss_relative_error %10.3e

quietly count
local total_tests = r(N)
quietly count if status == "pass"
local passed_tests = r(N)
quietly count if status == "fail"
local failed_tests = r(N)
quietly count if status == "skip"
local skipped_tests = r(N)

display as text _newline "Referee validation summary"
display as text "  total:   " as result `total_tests'
display as text "  passed:  " as result `passed_tests'
display as text "  failed:  " as result `failed_tests'
display as text "  skipped: " as result `skipped_tests'
tabulate status comparator, missing
capture noisily tabstat fereg_seconds alt_seconds time_ratio if status == "pass", ///
    by(comparator) statistics(n mean p50 min max) columns(statistics)

if `failed_tests' | `skipped_tests' {
    display as text _newline "Non-passing tests:"
    list test_id comparator status rc_fereg rc_alt rc_fereg_timing       ///
        rc_alt_timing failed_checks unchecked_common_e if status != "pass", ///
        noobs abbreviate(32)
}

capture program drop __frg_manual_compare
capture program drop __frg_manual_expected_error
capture program drop __frg_manual_saved_outputs
capture program drop __frg_manual_postestimation
capture mata: mata drop __frg_manual_matrix_distance()
foreach global_name in FRM_HAVE_RH FRM_REPS FRM_TEST FRM_CAT FRM_ALT     ///
    FRM_NEED_RH FRM_WEIGHT FRM_COEFS FRM_EXACT FRM_NUMERIC FRM_NONCOMP ///
    FRM_INDEX FRM_FCMD FRM_ACMD FRM_EXPECT_F FRM_EXPECT_A FRM_THREADS {
    capture macro drop `global_name'
}
set processors `original_processors'

local final_rc 0
if `failed_tests' local final_rc 459
if `require_reghdfe' & `skipped_tests' local final_rc 459

if `final_rc' {
    display as error "FEREG_REFEREE_SUITE|FAIL|tests=`total_tests'|passed=`passed_tests'|failed=`failed_tests'|skipped=`skipped_tests'"
    if `strict' exit `final_rc'
}
else if `skipped_tests' {
    display as text "FEREG_REFEREE_SUITE|INCOMPLETE|tests=`total_tests'|passed=`passed_tests'|failed=0|skipped=`skipped_tests'"
}
else {
    display as result "FEREG_REFEREE_SUITE|PASS|tests=`total_tests'|passed=`passed_tests'|failed=0|skipped=0"
}
