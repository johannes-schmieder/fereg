version 18.0
clear all
set obs 40
gen long id = _n
gen double x = (_n-20)/7
gen byte g = mod(_n,4)+1
gen byte h = mod(_n,3)+1
tempvar touse
gen byte `touse' = 1
local before_k = c(k)

fereg_parse c.x i.g i.g#c.x i.g##i.h, touse(`touse')
return list
matrix list r(model_terms)
matrix list r(model_components)
assert r(n_model_terms) > 5
assert r(n_model_components) >= r(n_model_terms)
assert r(model_wire_major) == 2
assert strpos("`r(source_vars)'", "x") > 0
assert strpos("`r(source_vars)'", "g") > 0
assert strpos("`r(source_vars)'", "h") > 0
confirm variable id x g h
assert c(k) == `before_k'
fereg_parse ibn.g, touse(`touse')
assert strpos("`r(coefficient_names)'", "bn.g") > 0
di as result "ordinary_factor_terms.do: PASS"
di as result "END_FIXTURE|ordinary_factor_terms|rc=0"
