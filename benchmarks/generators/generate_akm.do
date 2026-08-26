version 18.0
args n workers firms periods mobility assignment seed output
set more off

if `"`n'"' == "" local n 100000
if `"`workers'"' == "" local workers 10000
if `"`firms'"' == "" local firms 500
if `"`periods'"' == "" local periods 10
if `"`mobility'"' == "" local mobility .15
if `"`assignment'"' == "" local assignment "akm"
if `"`seed'"' == "" local seed 20260825
if `"`output'"' == "" local output "benchmark_akm.dta"

if !inlist(`"`assignment'"', "akm", "iid") {
    display as error "assignment must be akm or iid"
    exit 198
}
if `n' <= 0 | `workers' <= 0 | `firms' <= 0 | `periods' <= 0 {
    display as error "AKM dimensions must be positive"
    exit 198
}
if `mobility' < 0 | `mobility' > 1 {
    display as error "mobility must be in [0,1]"
    exit 198
}
if `"`assignment'"' == "akm" & `n' != `workers' * `periods' {
    display as error "AKM observations must equal workers times periods"
    exit 198
}

clear
set rng mt64
set seed `seed'

if `"`assignment'"' == "akm" {
    set obs `workers'
    generate long worker = _n
    expand `periods'
    bysort worker: generate int year = 2000 + _n - 1
    bysort worker (year): generate long firm = ceil(runiform() * `firms') if _n == 1
    bysort worker (year): replace firm = ///
        cond(runiform() < `mobility', ceil(runiform() * `firms'), firm[_n-1]) ///
        if _n > 1
}
else {
    set obs `n'
    generate long worker = ceil(runiform() * `workers')
    generate long firm = ceil(runiform() * `firms')
    generate int year = 2000 + mod(_n - 1, `periods')
}

bysort worker: generate double worker_fe = rnormal(0, .4) if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm: generate double firm_fe = rnormal(0, .2) if _n == 1
bysort firm: replace firm_fe = firm_fe[1]
generate double x1 = rnormal()
generate double x2 = rnormal()
generate double y = 2 + worker_fe + firm_fe + .2*x1 - .1*x2 + rnormal()
drop worker_fe firm_fe
compress
sort worker year
save `"`output'"', replace

display as result "AKM_DATASET|assignment=`assignment'|n=" _N ///
    "|workers=`workers'|firms=`firms'|periods=`periods'|mobility=`mobility'|seed=`seed'"
display as result "END_AKM_GENERATOR|rc=0"
exit 0
