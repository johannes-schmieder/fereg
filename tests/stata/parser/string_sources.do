version 18.0
clear all
input str12 worker str12 firm int year double x
"w#1" "firm-a" 2020 1
"w#2" "firm#a" 2020 2
"w#1" "firm-a" 2021 3
"w#2" "firm#a" 2021 4
end

stfixest_parse c.x, absorb(worker firm) cluster(firm#year worker)
assert r(n_absorb_terms) == 2
assert r(n_cluster_terms) == 2
assert strpos("`r(source_vars)'", "worker") > 0
assert strpos("`r(source_vars)'", "firm") > 0
assert c(k) == 4
di as result "string_sources.do: PASS"
