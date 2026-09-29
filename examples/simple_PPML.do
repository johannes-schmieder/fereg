version 18.0

local feppml_command "feppml"

clear all
set more off
set seed 12345

* Run this file from the fereg repository root.
adopath + "./stata"
adopath + "./dist"

* Simulate a balanced directed trade panel.
local Ncountries = 40
local T = 4

set obs `Ncountries'
generate int exporter = _n
expand `Ncountries'
bysort exporter: generate int importer = _n
drop if exporter == importer

expand `T'
bysort exporter importer: generate int year = 2018 + _n - 1
egen long pairid = group(exporter importer)

* Bilateral geography is fixed over time.
bysort pairid: generate double log_distance = ///
    log(300 + 9700 * runiform()) if _n == 1
bysort pairid: replace log_distance = log_distance[1]

bysort pairid: generate byte border = runiform() < .10 if _n == 1
bysort pairid: replace border = border[1]

* A subset of country pairs enters an agreement in 2020.
generate byte trade_agreement = ///
    year >= 2020 & mod(exporter + 2 * importer, 7) == 0

* Exporter-year and importer-year fixed effects.
bysort exporter year: generate double exporter_year_fe = ///
    rnormal(0, .25) if _n == 1
bysort exporter year: replace exporter_year_fe = exporter_year_fe[1]

bysort importer year: generate double importer_year_fe = ///
    rnormal(0, .25) if _n == 1
bysort importer year: replace importer_year_fe = importer_year_fe[1]

* Conditional mean for bilateral trade flows.
generate double eta = 6.0 - .55 * log_distance + .30 * border + ///
    .40 * trade_agreement + exporter_year_fe + importer_year_fe
generate long trade = rpoisson(exp(eta))

order exporter importer year pairid trade log_distance border ///
    trade_agreement
sort exporter importer year

summarize trade log_distance border trade_agreement

eststo clear
timer clear

* Established Stata comparator.
timer on 1
quietly ppmlhdfe trade log_distance border trade_agreement, ///
    absorb(exporter#year importer#year) vce(cluster pairid)
timer off 1
quietly timer list
local runtime = r(t1)
eststo ppmlhdfe
estadd scalar runtime = `runtime', replace : ppmlhdfe

scalar comparison_N = e(N)
scalar comparison_b_distance = _b[log_distance]
scalar comparison_b_border = _b[border]
scalar comparison_b_agreement = _b[trade_agreement]
scalar comparison_se_distance = _se[log_distance]
scalar comparison_se_border = _se[border]
scalar comparison_se_agreement = _se[trade_agreement]

* Candidate fereg PPML implementation.
timer on 2
quietly `feppml_command' trade log_distance border trade_agreement, ///
    absorb(exporter#year importer#year) vce(cluster pairid) ///
    separation(none) threads(8) timeit
timer off 2
quietly timer list
local runtime = r(t2)
eststo feppml
estadd scalar runtime = `runtime', replace : feppml

assert e(converged) == 1
assert e(N) == comparison_N

scalar max_b_difference = max( ///
    abs(_b[log_distance] - comparison_b_distance), ///
    abs(_b[border] - comparison_b_border), ///
    abs(_b[trade_agreement] - comparison_b_agreement))
scalar max_se_difference = max( ///
    abs(_se[log_distance] - comparison_se_distance), ///
    abs(_se[border] - comparison_se_border), ///
    abs(_se[trade_agreement] - comparison_se_agreement))

display as text "Maximum coefficient difference: " ///
    as result %12.4e max_b_difference
display as text "Maximum standard-error difference: " ///
    as result %12.4e max_se_difference

assert max_b_difference < 1e-6
assert max_se_difference < 1e-5

esttab ppmlhdfe feppml, ///
    keep(log_distance border trade_agreement) ///
    se star(* 0.10 ** 0.05 *** 0.01) b(4) ///
    stats(runtime N, labels("Time (s)" "Observations") fmt(3 0))

matrix list e(feppml_runtime)

exit 0
