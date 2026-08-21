version 18.0
args case_do capture_file reghdfe_ado_dir ftools_ado_dir require_ado_dir manifest_sha256
set more off

foreach required in case_do capture_file reghdfe_ado_dir ftools_ado_dir require_ado_dir manifest_sha256 {
    if `"``required''"' == "" {
        di as error "baseline_reghdfe.do requires an isolated runner argument: `required'"
        exit 198
    }
}

* The Python launcher has already verified that all three directories are clean,
* exact-commit checkouts outside the repository. Stata independently proves
* that its resolver sees those exact ado files.
adopath ++ `"`ftools_ado_dir'"'
adopath ++ `"`require_ado_dir'"'
adopath ++ `"`reghdfe_ado_dir'"'

capture findfile reghdfe.ado
if _rc {
    di as error "isolated reghdfe.ado was not found"
    exit 111
}
local found_reghdfe = subinstr(`"`r(fn)'"', "\", "/", .)
local expected_reghdfe = subinstr(`"`reghdfe_ado_dir'/reghdfe.ado"', "\", "/", .)
if `"`found_reghdfe'"' != `"`expected_reghdfe'"' {
    di as error "reghdfe resolved outside the verified checkout"
    exit 603
}

capture findfile ftools.ado
if _rc {
    di as error "isolated ftools.ado was not found"
    exit 111
}
local found_ftools = subinstr(`"`r(fn)'"', "\", "/", .)
local expected_ftools = subinstr(`"`ftools_ado_dir'/ftools.ado"', "\", "/", .)
if `"`found_ftools'"' != `"`expected_ftools'"' {
    di as error "ftools resolved outside the verified checkout"
    exit 603
}

capture findfile require.ado
if _rc {
    di as error "isolated require.ado was not found"
    exit 111
}
local found_require = subinstr(`"`r(fn)'"', "\", "/", .)
local expected_require = subinstr(`"`require_ado_dir'/require.ado"', "\", "/", .)
if `"`found_require'"' != `"`expected_require'"' {
    di as error "require resolved outside the verified checkout"
    exit 603
}

capture which ftools
if _rc {
    di as error "isolated ftools failed its version preflight"
    exit _rc
}
capture noisily ftools, compile
if _rc {
    di as error "isolated tracked ftools sources failed private mlib compilation"
    exit _rc
}
capture noisily reghdfe, version
if _rc {
    di as error "isolated reghdfe failed its version preflight"
    exit _rc
}

capture noisily do `"`case_do'"' `"`capture_file'"' `"`manifest_sha256'"'
local capture_rc = _rc
if `capture_rc' != 0 {
    di as error "reference capture failed before its terminal sentinel"
    exit `capture_rc'
}

di as result "END_FIXTURE|baseline_reghdfe|rc=0"
