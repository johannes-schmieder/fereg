*! fereg_run 0.5.0 26aug2026
*! Run named examples embedded in fereg.sthlp.
program define fereg_run
    version 15.0

    syntax anything(name=example_name id="example name") using/

    preserve
    capture noisily _fereg_run_example `example_name' using `"`using'"'
    local example_rc = _rc
    capture restore
    local restore_rc = _rc
    if `restore_rc' {
        display as error "fereg_run could not restore the caller's data"
        exit 498
    }
    exit `example_rc'
end

program define _fereg_run_example
    version 15.0

    syntax anything(name=example_name id="example name") using/

    quietly {
        capture confirm file `"`using'"'
        if _rc {
            capture findfile `"`using'"'
            if _rc {
                noisily display as error ///
                    `"help file `using' was not found on the Stata adopath"'
                exit 601
            }
            local help_file `"`r(fn)'"'
        }
        else local help_file `"`using'"'

        infix str244 source_line 1-244 using `"`help_file'"', clear
        generate long source_number = _n

        count if strpos(source_line, ///
            "{* example_start - `example_name'}{...}")
        if r(N) != 1 {
            if r(N) == 0 noisily display as error ///
                `"example `example_name' was not found in `using'"'
            else noisily display as error ///
                `"example `example_name' has duplicate start markers"'
            exit 111
        }
        summarize source_number if strpos(source_line, ///
            "{* example_start - `example_name'}{...}"), meanonly
        local first_line = r(min) + 1

        summarize source_number if source_number >= `first_line' & ///
            strpos(source_line, "{* example_end}{...}"), meanonly
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

        keep in `first_line'/`last_line'
        replace source_line = subinstr(source_line, "{c -(}", "{", .)
        replace source_line = subinstr(source_line, "{c )-}", "}", .)
    }

    tempfile example_do
    outfile source_line using `"`example_do'"', noquote
    do `"`example_do'"'
end
