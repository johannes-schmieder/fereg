version 18.0
clear all
sysuse auto
capture noisily fereg_p
assert _rc == 301
capture noisily fereg_estat
assert _rc == 301
di as result "postestimation/fail_closed.do: PASS"
di as result "END_FIXTURE|fail_closed|rc=0"
