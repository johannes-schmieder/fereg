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

* Calibrated AKM boundary: N/G_high=3 and G_high/G_second=10.
__frg_make_panel, periods(3) workers(100) firms(10)
foreach mode in fast low {
    quietly fereg y x, absorb(worker firm) solver(auto) memory(`mode') poolsize(1) threads(2)
    assert e(fereg_solver)[1,1] == 1
    assert e(fereg_solver)[1,2] == 7
    assert e(fereg_solver)[1,15] == 0
    assert e(fereg_solver)[1,11] == 0
}

* Below the panel-depth boundary, auto goes directly to its certified LSMR
* route without paying for a MAP pilot.
__frg_make_panel, periods(2) workers(100) firms(10)
quietly fereg y x, absorb(worker firm) solver(auto) memory(fast) poolsize(1) threads(2)
assert e(fereg_solver)[1,1] == 1
assert e(fereg_solver)[1,2] == 3
assert e(fereg_solver)[1,15] == 0

* Near-balanced graph cardinalities use the same pilot-free LSMR route.
__frg_make_panel, periods(10) workers(100) firms(11)
quietly fereg y x, absorb(worker firm) solver(auto) memory(fast) poolsize(1) threads(2)
assert e(fereg_solver)[1,1] == 1
assert e(fereg_solver)[1,2] == 3
assert e(fereg_solver)[1,15] == 0

* The post-hierarchy safety guard rejects repeat-amplified iterative CMG
* hierarchies as a planned route decision. It does not reject an iterative
* all-tiny hierarchy, and explicit CMG remains unchanged.
program define __frg_make_component_panel
    syntax, Components(integer) [Balanced]
    clear
    set obs 99000
    generate long obs_id = _n
    generate long worker = ceil(_n / 3)
    bysort worker: generate byte period = _n - 1
    local tiny_components = cond("`balanced'" == "", `components' - 1, `components')
    local small_workers = `tiny_components' * 30
    local small_firms = `tiny_components' * 3
    local remaining_firms = 3300 - `small_firms'
    generate long firm = floor((worker - 1) / 30) * 3 + ///
        mod(mod(worker - 1, 3) + period, 3) + 1 if worker <= `small_workers'
    if "`balanced'" == "" {
        replace firm = `small_firms' + ///
            mod(mod(worker - `small_workers' - 1, `remaining_firms') + period, ///
            `remaining_firms') + 1 if worker > `small_workers'
    }
    assert !missing(firm)
    generate double x = mod((obs_id-1)*37 + 11, 997)/97 - 5
    generate double y = .7*x + sin(worker * .13) + cos(firm * .29) + ///
        (mod((obs_id-1)*53 + 7, 101) - 50)*.001
end

__frg_make_component_panel, components(800)
quietly fereg y x, absorb(worker firm) solver(auto) threads(2)
assert e(fereg_solver)[1,1] == 1
assert e(fereg_solver)[1,2] == 3
assert e(fereg_solver)[1,11] == 0
assert e(fereg_solver)[1,12] == 0
assert e(fereg_solver)[1,15] == 0

quietly fereg y x, absorb(worker firm) solver(cmg) threads(2)
assert e(fereg_solver)[1,2] == 7

__frg_make_component_panel, components(1100) balanced
quietly fereg y x, absorb(worker firm) solver(auto) threads(2)
assert e(fereg_solver)[1,1] == 1
assert e(fereg_solver)[1,2] == 7
assert e(fereg_solver)[1,11] == 0
assert e(fereg_solver)[1,12] == 0
assert e(fereg_solver)[1,15] == 0

* Nine nuisance columns remain eligible, including when the low-cardinality FE
* is listed first. Ten nuisance columns cross to direct LSMR.
__frg_make_panel, periods(8) workers(100) firms(10)
generate byte nuisance10 = 1 + mod(_n - 1, 10)
quietly fereg y x, absorb(nuisance10 worker firm) solver(auto) memory(fast) poolsize(1) threads(2)
assert e(fereg_solver)[1,2] == 7
assert e(fereg_solver)[1,15] == 0

generate byte nuisance11 = 1 + mod(_n - 1, 11)
quietly fereg y x, absorb(nuisance11 worker firm) solver(auto) memory(fast) poolsize(1) threads(2)
assert e(fereg_solver)[1,2] == 3
assert e(fereg_solver)[1,15] == 0

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
