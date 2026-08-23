version 18.0
clear all
set more off

timer clear 1
timer on 1
forvalues repetition = 1/10 {
    capture program drop __stfixest_plugin
    capture noisily stfixest, selftest threads(4)
    assert _rc == 0
}
timer off 1
timer list 1

display as result "STFIXEST SMALL LIFECYCLE BENCHMARK PASS"
display as result "END_FIXTURE|stata_benchmark_small|rc=0"
