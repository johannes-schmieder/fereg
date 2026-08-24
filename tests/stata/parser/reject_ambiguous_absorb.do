version 18.0
clear all
set obs 20
gen byte a = mod(_n,2)
gen byte b = mod(_n,3)
gen double x = _n
tempvar touse
gen byte `touse' = 1
capture noisily fereg_parse c.x, touse(`touse') absorb(a##b)
assert _rc == 198
di as result "reject_ambiguous_absorb.do: PASS"
di as result "END_FIXTURE|reject_ambiguous_absorb|rc=0"
