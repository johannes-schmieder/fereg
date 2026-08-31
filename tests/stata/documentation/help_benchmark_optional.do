version 19.0
clear all
set more off

capture which reghdfe
if _rc {
    di as error "help_benchmark_optional.do requires reghdfe"
    exit 499
}

fereg_run benchmark using fereg.sthlp
di as result "END_FIXTURE|help_benchmark_optional|rc=0"
