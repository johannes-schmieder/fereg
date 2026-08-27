version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'

program define __frg_make_panel
    syntax, Periods(integer) Workers(integer) Firms(integer)
    clear
    set obs `workers'
    generate long worker = _n
    expand `periods'
    bysort worker: generate byte period = _n
    generate int firm = 1 + mod(worker + period, `firms')
    generate double x = mod((_n-1)*37 + 11, 997)/97 - 5
    generate double y = .7*x + sin(worker * .13) + cos(firm * .29) + ///
        (mod((_n-1)*53 + 7, 101) - 50)*.001
end

* Frozen favorable boundary: N/G_high=8 and G_high/G_second=10.
__frg_make_panel, periods(8) workers(100) firms(10)
foreach mode in fast low {
    quietly fereg y x, absorb(worker firm) solver(auto) memory(`mode') poolsize(1) threads(2)
    assert e(fereg_solver)[1,1] == 1
    assert e(fereg_solver)[1,2] == 7
    assert e(fereg_solver)[1,15] == 0
    assert e(fereg_solver)[1,11] == 0
}

* One observation per high-cardinality level below the frozen boundary stays
* on the existing certified auto route.
__frg_make_panel, periods(7) workers(100) firms(10)
quietly fereg y x, absorb(worker firm) solver(auto) memory(fast) poolsize(1) threads(2)
assert e(fereg_solver)[1,1] == 1
assert e(fereg_solver)[1,2] != 7

* Near-balanced graph cardinalities also remain on the existing route.
__frg_make_panel, periods(10) workers(100) firms(11)
quietly fereg y x, absorb(worker firm) solver(auto) memory(fast) poolsize(1) threads(2)
assert e(fereg_solver)[1,1] == 1
assert e(fereg_solver)[1,2] != 7

* Regression for tests/manual/simple_AKM.do: ordinary FE certification at the
* public tolerance was not strong enough for the downstream OLS score
* certificate. The CMG graph system now solves more tightly without changing
* the public reconstruction contract or adding a second fit.
clear
set seed 12345
set obs 10000
generate long worker = _n
expand 3
bysort worker: generate int year = 2000 + _n - 1
bysort worker (year): generate int firm = ceil(runiform()*500) if _n == 1
bysort worker (year): replace firm = ///
    cond(runiform() < .15, ceil(runiform()*500), firm[_n-1]) if _n > 1
bysort worker: generate double worker_fe = rnormal(0,.4) if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm: generate double firm_fe = rnormal(0,.2) if _n == 1
bysort firm: replace firm_fe = firm_fe[1]
generate double experience = year - 2000 + runiform()*5
generate double exp2 = experience^2
generate double y = 2 + worker_fe + firm_fe + .03*experience - ///
    .0005*exp2 + rnormal(0,.2)
quietly fereg y experience exp2, ///
    absorb(worker_saved=worker firm_saved=firm) solver(cmg)
assert e(fereg_solver)[1,2] == 7
assert e(fereg_solver)[1,11] == 0
assert !missing(worker_saved, firm_saved)

di as result "END_FIXTURE|estimate_ols_auto_cmg_route|rc=0"
