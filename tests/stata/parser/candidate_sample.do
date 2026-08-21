version 18.0
clear all
set obs 100
gen double x = _n
gen byte g = cond(_n<=25,3,cond(_n>90,4,cond(_n<=50,1,2)))
replace x = . in 1/5
replace g = 5 in 90
replace x = . in 90
local if "if _n > 25"
local in "in 1/90"
marksample touse, novarlist
local before_k = c(k)

stfixest_parse c.x i.g, touse(`touse')
local marker "`r(touse)'"
assert r(n_model_terms) >= 2
assert substr("`marker'", 1, 2) == "__"
assert strpos("`r(coefficient_names)'", "3.g") == 0
assert strpos("`r(coefficient_names)'", "4.g") == 0
assert strpos("`r(coefficient_names)'", "5.g") > 0
quietly count if `marker'
assert r(N) == 65
assert c(k) == `before_k'
di as result "candidate_sample.do: PASS"
di as result "END_FIXTURE|candidate_sample|rc=0"
