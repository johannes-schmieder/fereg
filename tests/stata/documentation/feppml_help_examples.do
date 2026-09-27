version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
local feppml_help_source `"$FEREG_PPML_HELP_SOURCE"'
if strtrim(`"`feppml_help_source'"') == "" local feppml_help_source "feppml.sthlp"
clear all
set more off

capture program drop _fereg_ppml_help_run
capture program drop _fereg_ppml_help_inner

program define _fereg_ppml_help_run
    version 18.0
    syntax anything(name=example_name id="example name") using/, COMMAND(name)

    preserve
    capture noisily _fereg_ppml_help_inner `example_name' ///
        using `"`using'"', command(`command')
    local example_rc = _rc
    capture restore
    local restore_rc = _rc
    if `restore_rc' {
        display as error "feppml help runner could not restore the caller's data"
        exit 498
    }
    exit `example_rc'
end

program define _fereg_ppml_help_inner
    version 18.0
    syntax anything(name=example_name id="example name") using/, COMMAND(name)

    quietly {
        capture confirm file `"`using'"'
        if _rc {
            capture findfile `"`using'"'
            if _rc {
                noisily display as error ///
                    `"help source `using' was not found on the Stata adopath"'
                exit 601
            }
            local help_file `"`r(fn)'"'
        }
        else local help_file `"`using'"'

        infix str244 source_line 1-244 using `"`help_file'"', clear
        generate long source_number = _n
        replace source_line = strtrim(source_line)

        local start_marker `"{* example_start - `example_name'}{...}"'
        count if source_line == `"`start_marker'"'
        if r(N) != 1 {
            if r(N) == 0 noisily display as error ///
                `"example `example_name' was not found in `using'"'
            else noisily display as error ///
                `"example `example_name' has duplicate start markers"'
            exit 111
        }
        summarize source_number if source_line == `"`start_marker'"', meanonly
        local first_line = r(min) + 1

        summarize source_number if source_number >= `first_line' & ///
            source_line == "{* example_end}{...}", meanonly
        if missing(r(min)) {
            noisily display as error ///
                `"example `example_name' has no closing marker in `using'"'
            exit 111
        }
        local last_line = r(min) - 1
        if `last_line' < `first_line' {
            noisily display as error ///
                `"example `example_name' contains no executable code"'
            exit 111
        }
        count if inrange(source_number, `first_line', `last_line') & ///
            strpos(source_line, "{* example_start - ") == 1
        if r(N) {
            noisily display as error ///
                `"example `example_name' contains a nested start marker"'
            exit 111
        }

        keep in `first_line'/`last_line'
        count if strpos(source_line, "feppml ") == 1
        if r(N) == 0 {
            noisily display as error ///
                `"example `example_name' contains no literal feppml command"'
            exit 111
        }
        replace source_line = subinstr(source_line, "feppml ", ///
            "`command' ", 1) if strpos(source_line, "feppml ") == 1
        replace source_line = subinstr(source_line, "{c -(}", "{", .)
        replace source_line = subinstr(source_line, "{c )-}", "}", .)
    }

    tempfile example_do
    outfile source_line using `"`example_do'"', noquote
    do `"`example_do'"'
end

set obs 5
generate long original_id = _n
generate double original_value = _n / 10
quietly datasignature set, reset

foreach example in ppml_basic ppml_absorbed_outputs ppml_weights ///
    ppml_clusters ppml_separation ppml_time_series ppml_bias_correction {
    capture noisily _fereg_ppml_help_run `example' ///
        using `"`feppml_help_source'"', command(`feppml_test_command')
    assert _rc == 0
    quietly datasignature confirm
}

capture noisily _fereg_ppml_help_run missing_example ///
    using `"`feppml_help_source'"', command(`feppml_test_command')
assert _rc == 111
quietly datasignature confirm

capture noisily _fereg_ppml_help_run ppml_basic ///
    using missing_feppml_help.sthlp, command(`feppml_test_command')
assert _rc == 601
quietly datasignature confirm

capture program drop _fereg_ppml_help_inner
capture program drop _fereg_ppml_help_run

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_help_examples|rc=0"
