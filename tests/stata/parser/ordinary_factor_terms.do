version 18.0
clear all
set obs 40
gen long id = _n
gen double x = (_n-20)/7
gen byte g = mod(_n,4)+1
gen byte h = mod(_n,3)+1

stfixest_parse c.x i.g i.g#c.x i.g##i.h ib2.g
return list
matrix list r(model_terms)
matrix list r(model_components)
assert r(n_model_terms) > 5
assert r(n_model_components) >= r(n_model_terms)
assert strpos("`r(source_vars)'", "x") > 0
assert strpos("`r(source_vars)'", "g") > 0
assert strpos("`r(source_vars)'", "h") > 0
confirm variable id x g h
assert c(k) == 4
di as result "ordinary_factor_terms.do: PASS"
di as result "END_FIXTURE|ordinary_factor_terms|rc=0"
