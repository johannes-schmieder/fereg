version 18.0
clear all
set more off
set linesize 255
set rng mt64
set processors 8
adopath ++ "/Users/johannes/Git/fereg/stata"
adopath ++ "/Users/johannes/Git/fereg/dist"
set seed 20263836
set obs 22500
generate long obs_id = _n
generate long worker = ceil(_n / 5)
bysort worker: generate int period = _n
bysort worker (period): generate long firm = mod(7919*worker - 1, 225) + 1 if _n == 1
bysort worker (period): replace firm = cond(runiform() < 0.02, mod(104729*worker + 7919*period - 1, 225) + 1, firm[_n-1]) if _n > 1
generate long county = mod(firm + 17*worker + 1049*period - 1, 12) + 1
bysort worker: generate double __worker_fe = rnormal(0,.4) if _n == 1
bysort worker: replace __worker_fe = __worker_fe[1]
bysort firm: generate double __firm_fe = rnormal(0,.2) if _n == 1
bysort firm: replace __firm_fe = __firm_fe[1]
bysort county: generate double __third_fe = rnormal(0,.1) if _n == 1
bysort county: replace __third_fe = __third_fe[1]
generate double __xb = 0
generate double x1 = rnormal()
replace __xb = __xb + (0.29999999999999999)*x1
generate double x2 = rnormal()
replace __xb = __xb + (-0.10000000000000001)*x2
generate double x3 = rnormal()
replace __xb = __xb + (0.099999999999999992)*x3
generate double x4 = rnormal()
replace __xb = __xb + (-0.050000000000000003)*x4
generate double y = 1 + __xb + __worker_fe + __firm_fe + rnormal(0,.5)
replace y = y + __third_fe
sort obs_id
assert _N == 22500
display as result "TRACE_BEGIN|case=cliff_n22500"
capture noisily fereg y x1 x2 x3 x4, absorb(worker firm county) dof(pairwise) solver(auto) threads(8) tolerance(1e-08) iterate(16000)
local __rc = _rc
display as result "TRACE_END|case=cliff_n22500|rc=" `__rc'
exit 0
