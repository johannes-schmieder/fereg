version 18.0
set more off
capture which ppmlhdfe
if _rc {
    di as error "Pinned ppmlhdfe must be installed before capturing this baseline"
    exit 111
}
local data "benchmark_panel.dta"
capture confirm file "`data'"
if _rc do benchmarks/generators/generate_panel.do 1000000 100000 10000 10 20260820 "`data'"
use "`data'", clear
log using benchmarks/reports/baseline_ppmlhdfe.log, text replace
ppmlhdfe, version
timer clear 1
timer on 1
ppmlhdfe y_count c.x1 c.x2 i.year [pw=weight], absorb(worker firm) vce(cluster firm)
timer off 1
timer list 1
ereturn list
log close
