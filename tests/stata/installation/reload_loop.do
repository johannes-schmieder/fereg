version 18.0
clear all
set more off
forvalues repetition = 1/100 {
    capture program drop __stfixest_plugin
    quietly stfixest, selftest threads(2)
}
di as result "reload_loop.do: PASS"
di as result "END_FIXTURE|reload_loop|rc=0"
