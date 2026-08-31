/* Generate one deterministic low-mobility AKM dataset outside measured calls. */
version 19.0
clear all
set more off
set linesize 255

args output_path n seed
if `"`output_path'"' == "" | `"`n'"' == "" | `"`seed'"' == "" {
    display as error "output_path, n, and seed are required"
    exit 198
}
local workers = `n' / 10
local firms = `workers' / 20
assert `workers' == floor(`workers')
assert `firms' == floor(`firms')

set rng mt64
set seed `seed'
quietly set obs `workers'
generate long worker = _n
quietly expand 10
bysort worker: generate byte period = _n
bysort worker (period): generate long firm = mod(worker - 1, `firms') + 1 if _n == 1
bysort worker (period): replace firm = cond(runiform() < .02, ///
    runiformint(1, `firms'), firm[_n-1]) if _n > 1

bysort worker: generate double __worker_fe = rnormal(0, .4) if _n == 1
bysort worker: replace __worker_fe = __worker_fe[1]
bysort firm: generate double __firm_fe = rnormal(0, .2) if _n == 1
bysort firm: replace __firm_fe = __firm_fe[1]
forvalues j = 1/10 {
    generate double x`j' = rnormal()
}
generate double __xb = 0
forvalues j = 1/10 {
    replace __xb = __xb + (.30/`j')*x`j'
}
generate double y = 2 + __worker_fe + __firm_fe + __xb + rnormal()
drop __worker_fe __firm_fe __xb

assert _N == `n'
bysort worker: generate long __count = _N
quietly summarize __count, meanonly
assert r(min) == 10 & r(max) == 10
drop __count
bysort firm: generate long __count = _N
quietly summarize __count, meanonly
assert r(min) >= 20
drop __count
sort worker period
quietly compress worker firm period
save `"`output_path'"', replace

display as result "PAPER_AKM_GENERATE_SUCCESS|n=`n'|workers=`workers'|firms=`firms'"
exit 0
