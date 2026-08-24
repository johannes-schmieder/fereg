version 18.0
clear all
set obs 120
gen long worker = ceil(_n/4)
gen long firm = mod(_n,11)+1
gen int year = 2000 + mod(_n,6)
gen double time = year-2000
gen double tenure = mod(_n,9)
gen double x = sin(_n)
tempvar touse
gen byte `touse' = 1
local before_k = c(k)

fereg_parse c.x, ///
    touse(`touse') ///
    absorb(worker firm#year firm##c.time firm#c.tenure worker##c.(time tenure)) ///
    cluster(firm year firm#year)
return list
matrix list r(absorb_terms)
matrix list r(absorb_components)
matrix list r(cluster_terms)
matrix list r(cluster_components)
assert r(n_absorb_terms) == 5
assert r(n_cluster_terms) == 3
assert c(k) == `before_k'
di as result "absorbed_slopes_and_clusters.do: PASS"
di as result "END_FIXTURE|absorbed_slopes_and_clusters|rc=0"
