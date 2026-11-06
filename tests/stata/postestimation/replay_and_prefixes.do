version 15.0
clear all
set more off

* Replay, Stata prefixes, the release label and the displayed header on every
* supported Stata release. Only Stata 15 syntax is used, so the fixture also
* runs unmodified on the oldest supported release.

fereg, selftest
local release `"`r(fereg_version)'"'
assert `"`release'"' == "0.5.1"

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

* Validate the printed statistics against stored results, including missing
* statistics, without changing coefficients, covariance, fit, DoF or sample.
program define check_model_display
    local command "`e(cmd)'"
    local ppml = "`command'" == "feppml"
    local prefix = cond(`ppml', "feppml", "fereg")
    local footer = e(N_hdfe) > 0
    tempname b v fit native_dof dof after handle
    tempvar sample
    matrix `b' = e(b)
    matrix `v' = e(V)
    matrix `fit' = e(`prefix'_fit)
    matrix `native_dof' = e(`prefix'_dof)
    if `footer' matrix `dof' = e(dof_table)
    generate byte `sample' = e(sample)
    local N : display %10.0fc e(N)
    local expected1 = itrim("Number of obs = `N'")
    if `ppml' {
        local dfr : display %10.0fc e(df_resid)
        local df = cond(missing(e(df_chi2)), e(df_m), e(df_chi2))
        local wald : display %10.2f e(chi2)
        local p : display %10.4f chi2tail(`df', e(chi2))
        local deviance : display %12.0g e(deviance)
        local ll : display %12.0g e(ll)
        local r2 : display %10.4f e(r2_p)
        local expected2 = itrim("Residual df = `dfr'")
        local expected3 = itrim("Wald chi2(`df') = `wald'")
        local expected4 = itrim("Prob > chi2 = `p'")
        local expected5 = itrim("Deviance = `deviance'")
        local expected6 = itrim("Log pseudolikelihood = `ll'")
        local expected7 = itrim("Pseudo R2 = `r2'")
        local plural = cond(e(ic) == 1, "", "s")
        local expected8 "(PPML estimator converged in `=e(ic)' iteration`plural')"
        local row 8
    }
    else {
        local F : display %10.2f e(F)
        local dfm : display %4.0f e(df_m)
        local dfr : display %7.0f e(df_r)
        local expected2 = itrim("F(`dfm',`dfr') = `F'")
        local p : display %10.4f Ftail(e(df_m), e(df_r), e(F))
        local expected3 = itrim("Prob > F = `p'")
        local row 3
        foreach stat in r2 r2_a r2_within rmse {
            local ++row
            local value : display %10.4f e(`stat')
            local label = cond("`stat'" == "r2", "R-squared", ///
                cond("`stat'" == "r2_a", "Adj R-squared", ///
                cond("`stat'" == "r2_within", "Within R-sq.", "Root MSE")))
            local expected`row' = itrim("`label' = `value'")
        }
    }
    local ++row
    local expected`row' "[90% conf. interval]"
    local nclusters = cond(missing(e(N_clustervars)), 0, e(N_clustervars))
    forvalues i = 1/`nclusters' {
        local ++row
        local value : display %10.0fc e(N_clust`i')
        local expected`row' = itrim("Number of clusters (`e(clustvar`i')') = `value'")
    }
    if `footer' {
        local names : rownames `dof'
        forvalues i = 1/`=rowsof(`dof')' {
            local ++row
            local name : word `i' of `names'
            local name = subinstr("`name'", "1.", "", .)
            forvalues j = 1/3 {
                local value`j' : display %10.0g `dof'[`i',`j']
            }
            local note = cond(`dof'[`i',5] == 1, "*", cond(`dof'[`i',4] == 1, "?", ""))
            local expected`row' = itrim("`name' | `value1' `value2' `value3' `note' |")
        }
    }
    forvalues i = 1/`row' {
        local found`i' 0
    }
    local found_footer 0
    tempfile output
    quietly log using `"`output'"', text replace name(model_summary)
    `command', level(90)
    quietly log close model_summary
    matrix `after' = e(b)
    assert mreldif(`b', `after') == 0
    matrix `after' = e(V)
    assert mreldif(`v', `after') == 0
    matrix `after' = e(`prefix'_fit)
    assert mreldif(`fit', `after') == 0
    matrix `after' = e(`prefix'_dof)
    assert mreldif(`native_dof', `after') == 0
    if `footer' {
        matrix `after' = e(dof_table)
        assert mreldif(`dof', `after') == 0
    }
    assert `sample' == e(sample)
    file open `handle' using `"`output'"', read text
    file read `handle' line
    while r(eof) == 0 {
        local line = itrim(strtrim(`"`macval(line)'"'))
        if `ppml' {
            foreach label in "R-squared" "Within R-sq." "Root MSE" "Prob > F" {
                assert strpos(`"`macval(line)'"', "`label'") == 0
            }
        }
        forvalues i = 1/`row' {
            if strpos(`"`macval(line)'"', `"`expected`i''"') local found`i' 1
        }
        if `"`macval(line)'"' == "Absorbed degrees of freedom:" local found_footer 1
        file read `handle' line
    }
    file close `handle'
    forvalues i = 1/`row' {
        if !`found`i'' di as error `"Missing display field: `expected`i''"'
        assert `found`i'' == 1
    }
    assert `found_footer' == `footer'
end

quietly fereg y x z, absorb(worker firm)
check_model_display
quietly fereg y x z, absorb(worker) vce(robust) noconstant
check_model_display
quietly fereg y x z, absorb(worker firm) vce(cluster worker firm)
check_model_display
quietly fereg y x z, absorb(worker##c.period firm)
check_model_display
quietly fereg y x z, absorb(worker firm period)
check_model_display
quietly fereg y x z
check_model_display
quietly fereg y
assert missing(e(F))
check_model_display
clonevar long_firm_identifier = firm
quietly fereg y x z, absorb(worker long_firm_identifier) vce(cluster long_firm_identifier)
check_model_display

* PPML uses its own fit statistics and z inference, including missing Wald
* statistics, weighted counts, absorbed slopes and clustered DoF markers.
quietly feppml count x z, absorb(worker firm)
check_model_display
quietly feppml count x z, absorb(worker firm) vce(cluster worker firm)
check_model_display
quietly feppml count x z, absorb(worker##c.period firm)
check_model_display
quietly feppml count x z [fw=w], absorb(firm)
check_model_display
quietly feppml count x z
check_model_display
quietly feppml count
assert missing(e(chi2))
check_model_display
quietly feppml count x z, absorb(worker long_firm_identifier) vce(cluster long_firm_identifier)
check_model_display

* Resampling replay does not label the original fit's Wald test as bootstrap inference.
bootstrap, reps(4) seed(1) nodots cluster(firm): feppml count x z, absorb(worker firm)
quietly log using `"`output'"', text replace name(prefix_display)
feppml
quietly log close prefix_display
file open `handle' using `"`output'"', read text
file read `handle' line
while r(eof) == 0 {
    assert strpos(`"`macval(line)'"', "HDFE PPML regression") == 0
    assert strpos(`"`macval(line)'"', "Absorbed degrees of freedom:") == 0
    file read `handle' line
}
file close `handle'

di as result "replay_and_prefixes.do: PASS"
di as result "END_FIXTURE|replay_and_prefixes|rc=0"
