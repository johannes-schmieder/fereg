version 18.0
clear all
set more off
use "${GENERATED_PREDICTOR_ROOT}/data/topology_rank-s31415927-warm.dta", clear
ereturn clear
capture noisily feppml y x1-x3 i.f##i.h, absorb(g1 f) threads(4) solver(auto) poolsize(10) vce(robust)
local rc=_rc
assert `rc'==430
capture matrix forbidden=e(b)
assert _rc!=0
display "TOPOLOGY_DIAGNOSTIC|scope=warm|rc=`rc'"
use "${GENERATED_PREDICTOR_ROOT}/data/topology_rank-s31415927-data.dta", clear
ereturn clear
capture noisily feppml y x1-x3 i.f##i.h, absorb(g1 f) threads(4) solver(auto) poolsize(10) vce(robust)
local rc=_rc
assert `rc'==430
capture matrix forbidden=e(b)
assert _rc!=0
display "TOPOLOGY_DIAGNOSTIC|scope=data|rc=`rc'"
display "END_FIXTURE|topology_failure_atomicity|checks=2|rc=0"
