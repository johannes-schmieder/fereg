version 18.0
args n workers firms years seed output
if "`n'" == "" local n 1000000
if "`workers'" == "" local workers 100000
if "`firms'" == "" local firms 10000
if "`years'" == "" local years 10
if "`seed'" == "" local seed 20260820
if "`output'" == "" local output "benchmark_panel.dta"
clear
set obs `n'
set seed `seed'
gen long worker = ceil(runiform()*`workers')
gen long firm = ceil(runiform()*`firms')
gen int year = 2000 + floor(runiform()*`years')
gen double x1 = rnormal()
gen double x2 = rnormal()
gen double weight = 0.5 + runiform()
gen double eta = .2*x1 - .1*x2 + .02*(year-2000)
gen double y_linear = eta + rnormal()
gen double mu = exp(min(eta,10))
gen double y_count = rpoisson(mu)
compress
save "`output'", replace
