version 18.0
clear all
set obs 24
gen byte g = mod(_n,3)+1
gen byte h = mod(_n,2)+1
gen double x = _n/7
gen double z = (_n-12)/5
tempvar touse
gen byte `touse' = _n > 2

fereg_parse c.x i.g i.g#c.x i.g##c.(x z) i.g##i.h c.x#c.z, ///
    touse(`touse')
local wire_major = r(model_wire_major)
local wire_minor = r(model_wire_minor)
local n_sources = r(n_sources)
local n_terms = r(n_model_terms)
local n_components = r(n_model_components)
local raw_operators `"`r(component_raw_operators)'"'
matrix model_sources = r(model_sources)
matrix model_terms = r(model_terms)
matrix model_components = r(model_components)
assert `wire_major' == 2
assert `wire_minor' == 1
assert `n_sources' == 4
assert `n_terms' > 10
assert `n_components' > `n_terms'
assert strpos(`"`raw_operators'"', "2:2o") > 0
confirm variable `touse'
quietly count if `touse'
assert r(N) == 22
matrix list model_sources
matrix list model_terms
matrix list model_components
di as result "WIRE_V2|sources=`n_sources'|terms=`n_terms'|components=`n_components'"

fereg_parse i.g#c.x, touse(`touse')
local direct_names `"`r(coefficient_names)'"'
local direct_base : list posof "1b.g#c.x" in direct_names
assert `direct_base' > 0
matrix direct_terms = r(model_terms)
assert direct_terms[`direct_base',6] == 0

fereg_parse i.g##c.(x z), touse(`touse')
local continuous_names `"`r(coefficient_names)'"'
local co_interaction : list posof "1b.g#co.x" in continuous_names
assert `co_interaction' > 0
matrix continuous_terms = r(model_terms)
assert continuous_terms[`co_interaction',6] == 1
assert strpos("`r(component_raw_operators)'", "2:co") > 0

fereg_parse i.g##i.h, touse(`touse')
assert strpos("`r(component_raw_operators)'", "2:2o") > 0

fereg_parse ibn.g, touse(`touse')
assert strpos("`r(coefficient_names)'", "bn.g") > 0

tempvar singleton_candidate
gen byte `singleton_candidate' = g == 3
fereg_parse i.g, touse(`singleton_candidate')
assert r(n_model_terms) == 1
assert strpos("`r(coefficient_names)'", "3.g") > 0
assert strpos("`r(coefficient_names)'", "b.g") == 0

fereg_parse, touse(`touse') emptymodel
assert r(n_sources) == 0
assert r(n_model_terms) == 0
assert r(n_model_components) == 0
matrix empty_sources = r(model_sources)
matrix empty_terms = r(model_terms)
matrix empty_components = r(model_components)
assert rowsof(empty_sources) == 1 & colsof(empty_sources) == 2
assert rowsof(empty_terms) == 1 & colsof(empty_terms) == 8
assert rowsof(empty_components) == 1 & colsof(empty_components) == 13
assert empty_sources[1,1] == 0 & empty_sources[1,2] == 0
assert empty_terms[1,1] == 0 & empty_terms[1,8] == 0
assert empty_components[1,1] == 0 & empty_components[1,13] == 0

fereg_parse, touse(`touse') emptymodel absorb(g h)
assert r(n_absorb_terms) == 2
assert r(n_absorb_components) == 2
assert `"`r(absorb_label_registry)'"' == "1:g|1:h"
assert `"`r(absorb_saved_name_registry)'"' == "0:|0:"
matrix absorb_terms = r(absorb_terms)
matrix absorb_components = r(absorb_components)
assert rowsof(absorb_terms) == 2 & colsof(absorb_terms) == 4
assert rowsof(absorb_components) == 2 & colsof(absorb_components) == 4
assert absorb_terms[1,1] == 1 & absorb_terms[2,1] == 2
assert absorb_components[1,4] != absorb_components[2,4]
di as result "model_wire_v2_capture.do: PASS"
di as result "END_FIXTURE|model_wire_v2_capture|rc=0"
