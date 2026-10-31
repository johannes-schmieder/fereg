version 15.0
clear all
set more off

* Replay, Stata prefixes, the release label and the displayed header on every
* supported Stata release. Only Stata 15 syntax is used, so the fixture also
* runs unmodified on the oldest supported release.

fereg, selftest
local release `"`r(fereg_version)'"'
assert strpos(`"`release'"', "0.5.0-beta.") == 1

set seed 20261031
set obs 600
generate long worker = ceil(_n/6)
bysort worker: generate byte period = _n
generate long firm = mod(worker*7 + period*3, 25) + 1
generate double x = rnormal()
generate double z = rnormal()
generate double y = .5*x - .2*z + mod(worker,9)/10 + mod(firm,5)/10 + rnormal()
generate double count = rpoisson(exp(1 + .3*x - .1*z + mod(firm,5)/10))
generate double w = 1 + mod(worker,3)

* Coefficients agree with independent dummy-variable fits.
fereg y x z i.period [aw=w], absorb(worker firm) vce(cluster firm)
assert `"`e(fereg_version)'"' == `"`release'"'
scalar fereg_x = _b[x]
scalar fereg_z = _b[z]
quietly areg y x z i.period i.firm [aw=w], absorb(worker) vce(cluster firm)
assert reldif(fereg_x, _b[x]) < 1e-8
assert reldif(fereg_z, _b[z]) < 1e-8

feppml count x z, absorb(worker firm) vce(robust)
assert `"`e(fereg_version)'"' == `"`release'"'
scalar feppml_x = _b[x]
scalar feppml_z = _b[z]
quietly poisson count x z i.worker i.firm, vce(robust)
assert reldif(feppml_x, _b[x]) < 1e-6
assert reldif(feppml_z, _b[z]) < 1e-6

* Replay redisplays the last results with Stata's display options.
fereg y x z, absorb(worker firm) vce(cluster firm)
estimates store ols
tempname b_ols
matrix `b_ols' = e(b)
fereg
fereg, level(90) cformat(%9.4f)
estimates replay ols
feppml count x z, absorb(worker firm)
estimates store ppml
feppml
feppml, level(90)
estimates replay ppml
capture noisily fereg
assert _rc == 301
estimates restore ols

* Other options without a variable list are rejected before e(cmd) matters.
foreach option in version doctor bogus {
    capture noisily fereg, `option'
    assert _rc == 198
    capture noisily feppml, `option'
    assert _rc == 198
}
capture noisily fereg, threads(2)
assert _rc == 198
capture noisily fereg, selftest bogus
assert _rc == 198
tempname b_after
matrix `b_after' = e(b)
assert mreldif(`b_ols', `b_after') == 0

* bootstrap: and jackknife: refit on resampled data and redisplay.
bootstrap, reps(4) seed(1) nodots cluster(firm): fereg y x z, absorb(worker firm)
assert `"`e(cmd)'"' == "fereg" & `"`e(prefix)'"' == "bootstrap"
assert e(N_reps) == 4
fereg
jackknife, cluster(firm) nodots: fereg y x z, absorb(worker firm)
assert `"`e(cmd)'"' == "fereg" & `"`e(prefix)'"' == "jackknife"
bootstrap, reps(4) seed(1) nodots cluster(firm): feppml count x z, absorb(worker firm)
assert `"`e(cmd)'"' == "feppml" & `"`e(prefix)'"' == "bootstrap"
feppml
ereturn clear
capture noisily fereg
assert _rc == 301
capture noisily feppml
assert _rc == 301

* The header states the passed accuracy check; saved outputs print no
* "missing values generated" messages.
tempfile output
quietly log using `"`output'"', text replace name(fereg_display)
fereg y x z, absorb(fe_w=worker fe_f=firm) residuals(res)
feppml count x z, absorb(worker firm) d(dd) mu(mm)
quietly log close fereg_display
tempname handle
local header 0
local missing 0
file open `handle' using `"`output'"', read text
file read `handle' line
while r(eof) == 0 {
    if strpos(`"`macval(line)'"', "Fixed-effect solver:") == 1 & ///
       strpos(`"`macval(line)'"', "Accuracy check: passed (tolerance 1e-08)") ///
        local header 1
    if strpos(`"`macval(line)'"', "missing values generated") local missing 1
    file read `handle' line
}
file close `handle'
assert `header' == 1
assert `missing' == 0
confirm variable res fe_w fe_f dd mm

di as result "replay_and_prefixes.do: PASS"
di as result "END_FIXTURE|replay_and_prefixes|rc=0"
