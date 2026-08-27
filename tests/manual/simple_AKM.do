
clear all
set seed 12345

adopath + /Users/johannes/Git/fereg/stata
adopath + /Users/johannes/Git/fereg/dist

global GTOOLS_BETA = "I KNOW WHAT I AM DOING"

* Size
local scale = 1
local Nworkers = `scale'*10000
local Nfirms   = `scale'*500
local T        = 3

* Worker-year panel
set obs `Nworkers'
gen workerid = _n
expand `T'
bys workerid: gen year = 2000 + _n - 1

* Firm assignment with worker mobility
bys workerid (year): gen firmid = ceil(runiform()*`Nfirms') if _n == 1
bys workerid (year): replace firmid = ///
    cond(runiform() < .15, ceil(runiform()*`Nfirms'), firmid[_n-1]) if _n > 1

* Worker and firm fixed effects
bys workerid: gen worker_fe = rnormal(0,.4) if _n == 1
bys workerid: replace worker_fe = worker_fe[1]

bys firmid: gen firm_fe = rnormal(0,.2) if _n == 1
bys firmid: replace firm_fe = firm_fe[1]

* Controls
gen experience = year - 2000 + runiform()*5
gen x = rnormal()

* Log wages
gen lnwage = 2 + worker_fe + firm_fe ///
    + .03*experience - .0005*experience^2 + .10*x + rnormal(0,.2)

order workerid firmid year lnwage experience x
sort workerid year

g exp2 = experience^2 
eststo clear
timer clear

timer on 1
quietly reghdfe lnwage experience exp2, absorb(what1=workerid fhat1=firmid)
timer off 1
quietly timer list
local runtime = r(t1)
eststo reghdfe
estadd scalar runtime = `runtime', replace : reghdfe

timer on 2
quietly fereg lnwage experience exp2, absorb(what2=workerid fhat2=firmid) solver(auto)
timer off 2
quietly timer list
local runtime = r(t2)
eststo fereg_auto
estadd scalar runtime = `runtime', replace : fereg_auto

timer on 3
quietly fereg lnwage experience exp2, ///
    absorb(what_cmg=workerid fhat_cmg=firmid) solver(CMG)
timer off 3
quietly timer list
local runtime = r(t3)
eststo fereg_cmg
estadd scalar runtime = `runtime', replace : fereg_cmg

timer on 4
quietly greg lnwage experience exp2, ///
    absorb(workerid firmid) alphas(what3 fhat3)
timer off 4
quietly timer list
local runtime = r(t4)
eststo greg
estadd scalar runtime = `runtime', replace : greg

timer on 5
quietly areg lnwage experience exp2, absorb(workerid firmid)
timer off 5
quietly timer list
local runtime = r(t5)
eststo areg
estadd scalar runtime = `runtime', replace : areg

esttab, se star(* 0.10 ** 0.05 *** 0.01) b(a2) ///
    stats(runtime, labels("Time (s)") fmt(3))

exit



timer on 4
reghdfe lnwage experience exp2, absorb(workerid )
timer off 4

timer on 5
fereg lnwage experience exp2, absorb(workerid )
timer off 5

timer on 6
greg lnwage experience exp2, absorb(workerid ) 
timer off 6


timer list

exit 

set processors 8

* Warm up; omit saved FEs
quietly reghdfe lnwage experience exp2, absorb(workerid firmid)
quietly fereg lnwage experience exp2, absorb(workerid firmid) ///
    threads(8) solver(lsmr)

timer clear
forvalues r = 1/5 {
    timer on 1
    quietly reghdfe lnwage experience exp2, absorb(workerid firmid)
    timer off 1

    timer on 2
    quietly fereg lnwage experience exp2, absorb(workerid firmid) ///
        threads(8) solver(lsmr)
    timer off 2
	
	timer on 3
    quietly fereg lnwage experience exp2, absorb(workerid firmid) ///
        threads(8) solver(auto)
    timer off 3
}
timer list

fereg lnwage experience exp2, absorb(workerid firmid) ///
    threads(8) solver(auto) timeit

matrix list e(fereg_solver)
matrix list e(fereg_runtime)
