version 18.0
clear all
sysuse auto
capture noisily stfixest_p
assert _rc == 498
capture noisily stfixest_estat
assert _rc == 498
di as result "postestimation/fail_closed.do: PASS"
