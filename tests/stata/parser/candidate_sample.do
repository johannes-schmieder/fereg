version 18.0
clear all
set obs 100
gen double x = _n
gen byte g = cond(_n<=50,1,2)
replace x = . in 1/5
tempvar touse
gen byte `touse' = 0
replace `touse' = 1 if _n > 25 in 1/90

stfixest_parse c.x i.g, touse(`touse')
local marker "`r(touse)'"
assert r(n_model_terms) >= 2
quietly count if `marker'
assert r(N) == 65
assert c(k) == 3
di as result "candidate_sample.do: PASS"
di as result "END_FIXTURE|candidate_sample|rc=0"
