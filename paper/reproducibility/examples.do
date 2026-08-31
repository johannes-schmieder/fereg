version 18.0
clear all
set more off

capture noisily fereg, version
if _rc {
    display as error "Install fereg and its platform plugin before running."
    exit _rc
}

* Example 1: two-way fixed effects and firm-clustered inference.
clear
set seed 2026082601
set obs 600
generate long worker = ceil(_n/6)
bysort worker: generate byte period = _n
generate long firm = mod(worker + 7*period, 30) + 1
generate double x = rnormal()
bysort worker: generate double worker_fe = rnormal() if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm: generate double firm_fe = .7*rnormal() if _n == 1
bysort firm: replace firm_fe = firm_fe[1]
generate double y = 1 + .8*x + worker_fe + firm_fe + rnormal()
fereg y x, absorb(worker firm) vce(cluster firm)
estimates store basic

* Example 2: factors, weights, multiway clustering, and saved outputs.
clear
set seed 2026082604
set obs 1200
generate long worker = ceil(_n/10)
bysort worker: generate byte period = _n
generate int year = 2010 + mod(period-1,5)
generate long firm = mod(worker + 7*period, 40) + 1
generate byte state = mod(firm-1,10) + 1
generate byte region = 1 + mod(state-1,4)
generate double x = rnormal()
generate double aw = .5 + runiform()
bysort worker: generate double worker_fe = rnormal() if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm: generate double firm_fe = .6*rnormal() if _n == 1
bysort firm: replace firm_fe = firm_fe[1]
generate double y = 1 + .6*x + .15*(region == 2) + worker_fe + ///
    firm_fe + .05*(year-2010) + rnormal()
fereg y x i.region [aw=aw], ///
    absorb(fe_worker=worker fe_firm=firm year) ///
    vce(cluster state#year worker) residuals(ehat)
predict double xb, xb
predict double d, d
predict double xbd, xbd
assert reldif(y, xbd + ehat) < 1e-8 if e(sample)

* Example 3: interacted fixed effects and heterogeneous slopes.
clear
set seed 2026082607
set obs 1200
generate long worker = ceil(_n/12)
bysort worker: generate byte period = _n
generate int year = 2015 + mod(period-1,6)
generate double trend = period - 6.5
generate long firm = mod(worker + 5*ceil(period/3), 25) + 1
generate double x = rnormal()
bysort worker: generate double worker_fe = rnormal() if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm year: generate double firm_year_fe = ///
    .5*rnormal() if _n == 1
bysort firm year: replace firm_year_fe = firm_year_fe[1]
bysort firm: generate double firm_slope = .08*rnormal() if _n == 1
bysort firm: replace firm_slope = firm_slope[1]
generate double y = 2 + .7*x + worker_fe + firm_year_fe + ///
    firm_slope*trend + rnormal()
fereg y x, absorb(worker firm#year firm#c.trend) vce(robust)

* Example 4: execution diagnostics and explicit low-memory planning.
clear
set seed 2026082610
set obs 200000
generate long worker = ceil(_n/8)
generate long firm = mod(_n + 17*worker, 12000) + 1
generate double x1 = rnormal()
generate double x2 = rnormal()
generate double y = .5*x1 - .2*x2 + rnormal()
fereg y x1 x2, absorb(worker firm) memory(low) poolsize(2) ///
    threads(4) deterministic timeit
matrix list e(fereg_solver)
matrix list e(fereg_runtime)
matrix list e(fereg_certificate)

display as result "FEREG_PAPER_EXAMPLES_SUCCESS"
