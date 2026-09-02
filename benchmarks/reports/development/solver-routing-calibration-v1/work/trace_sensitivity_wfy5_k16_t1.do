version 18.0
clear all
set more off
set linesize 255
set rng mt64
set processors 8
adopath ++ "/Users/johannes/Git/fereg/stata"
adopath ++ "/Users/johannes/Git/fereg/dist"
set seed 711784490
set obs 50000
generate long obs_id = _n
generate long worker = ceil(_n / 5)
bysort worker: generate int period = _n
bysort worker (period): generate long firm = mod(7919*worker - 1, 500) + 1 if _n == 1
bysort worker (period): replace firm = cond(runiform() < 0.02, mod(104729*worker + 7919*period - 1, 500) + 1, firm[_n-1]) if _n > 1
generate long year = mod(obs_id - 1, 5) + 1
bysort worker: generate double __worker_fe = rnormal(0,.4) if _n == 1
bysort worker: replace __worker_fe = __worker_fe[1]
bysort firm: generate double __firm_fe = rnormal(0,.2) if _n == 1
bysort firm: replace __firm_fe = __firm_fe[1]
bysort year: generate double __third_fe = rnormal(0,.1) if _n == 1
bysort year: replace __third_fe = __third_fe[1]
generate double __xb = 0
generate double x1 = rnormal()
replace __xb = __xb + (0.29999999999999999)*x1
generate double x2 = rnormal()
replace __xb = __xb + (-0.10000000000000001)*x2
generate double x3 = rnormal()
replace __xb = __xb + (0.099999999999999992)*x3
generate double x4 = rnormal()
replace __xb = __xb + (-0.050000000000000003)*x4
generate double x5 = rnormal()
replace __xb = __xb + (0.059999999999999998)*x5
generate double x6 = rnormal()
replace __xb = __xb + (-0.033333333333333333)*x6
generate double x7 = rnormal()
replace __xb = __xb + (0.042857142857142858)*x7
generate double x8 = rnormal()
replace __xb = __xb + (-0.025000000000000001)*x8
generate double x9 = rnormal()
replace __xb = __xb + (0.033333333333333333)*x9
generate double x10 = rnormal()
replace __xb = __xb + (-0.02)*x10
generate double x11 = rnormal()
replace __xb = __xb + (0.027272727272727271)*x11
generate double x12 = rnormal()
replace __xb = __xb + (-0.016666666666666666)*x12
generate double x13 = rnormal()
replace __xb = __xb + (0.023076923076923075)*x13
generate double x14 = rnormal()
replace __xb = __xb + (-0.014285714285714287)*x14
generate double x15 = rnormal()
replace __xb = __xb + (0.02)*x15
generate double x16 = rnormal()
replace __xb = __xb + (-0.012500000000000001)*x16
generate double y = 1 + __xb + __worker_fe + __firm_fe + rnormal(0,.5)
replace y = y + __third_fe
sort obs_id
assert _N == 50000
display as result "TRACE_BEGIN|case=sensitivity_wfy5_k16_t1"
capture noisily fereg y x1 x2 x3 x4 x5 x6 x7 x8 x9 x10 x11 x12 x13 x14 x15 x16, absorb(worker firm year) dof(pairwise) solver(auto) threads(1) tolerance(1e-08) iterate(16000)
local __rc = _rc
display as result "TRACE_END|case=sensitivity_wfy5_k16_t1|rc=" `__rc'
exit 0
