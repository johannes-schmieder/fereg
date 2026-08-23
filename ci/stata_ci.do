version 18
clear all
set more off
set varabbrev off

args suite_file status_file log_file profile repo_root run_root

capture log close _all
log using `"`log_file'"', text replace name(stata_ci)

display as text "STATA_CI_BEGIN profile=`profile'"
display as text "STATA_CI_VERSION " c(stata_version)
display as text "STATA_CI_EDITION " c(edition_real)
display as text "STATA_CI_OS " c(os)
display as text "STATA_CI_MACHINE " c(machine_type)

sysdir set PLUS `"`run_root'/stata-plus"'
sysdir set PERSONAL `"`run_root'/stata-personal"'
sysdir set OLDPLACE `"`run_root'/stata-oldplace"'
sysdir set SITE `"`run_root'/stata-site"'
cd `"`repo_root'"'

capture noisily do `"`suite_file'"'
local stata_rc = _rc
local ci_stata_version = c(stata_version)
local ci_stata_edition `"`c(edition_real)'"'
local ci_stata_os `"`c(os)'"'
local ci_stata_machine_type `"`c(machine_type)'"'
local ci_stata_processors = c(processors)
local ci_stata_born_date `"`c(born_date)'"'

if `stata_rc' == 0 {
    display as result "STATA_CI_SUITE_OK profile=`profile'"
}
else {
    display as error "STATA_CI_SUITE_FAILED profile=`profile' rc=`stata_rc'"
}

tempname status_handle
capture file open `status_handle' using `"`status_file'"', write text replace
local status_open_rc = _rc
if `status_open_rc' != 0 {
    display as error "STATA_CI_STATUS_OPEN_FAILED rc=`status_open_rc'"
    capture log close stata_ci
    exit `status_open_rc'
}

file write `status_handle' "schema_version=1" _n
file write `status_handle' "profile=`profile'" _n
file write `status_handle' "stata_rc=`stata_rc'" _n
file write `status_handle' "stata_version=`ci_stata_version'" _n
file write `status_handle' "stata_edition=`ci_stata_edition'" _n
file write `status_handle' "stata_os=`ci_stata_os'" _n
file write `status_handle' "stata_machine_type=`ci_stata_machine_type'" _n
file write `status_handle' "stata_processors=`ci_stata_processors'" _n
file write `status_handle' "stata_born_date=`ci_stata_born_date'" _n
file write `status_handle' "tests_passed=" (`stata_rc' == 0) _n
file write `status_handle' "tests_failed=" (`stata_rc' != 0) _n
file write `status_handle' "completed=1" _n
file close `status_handle'

capture log close stata_ci
exit `stata_rc'
