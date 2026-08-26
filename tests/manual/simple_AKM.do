clear
set seed 1000

clear all
set seed 12345

* Size
local Nworkers = 100000
local Nfirms   = 5000
local T        = 10

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
timer clear

timer on 1
reghdfe lnwage experience exp2, absorb(workerid firmid)
timer off 1

timer on 2
fereg lnwage experience exp2, absorb(workerid firmid) solver(lsmr) threads(8)
timer off 2

timer on 3
greg lnwage experience exp2, absorb(workerid firmid) 
timer off 3

timer on 4
reghdfe lnwage experience exp2, absorb(workerid )
timer off 4

timer on 5
fereg lnwage experience exp2, absorb(workerid ) solver(auto) threads(8)
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
