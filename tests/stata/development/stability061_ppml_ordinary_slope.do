version 15.0
clear all
set more off

* Exact ordinary-slope data from replay_and_prefixes.do. Preserve the y draw:
* although unused below, it fixes the RNG state before the Poisson counts.
* The model has 29 zero outcomes but a finite full-sample Poisson optimum.
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
quietly count if count == 0
assert r(N) == 29

* feppml's default is robust with N/(N-1), matching poisson, vce(robust).
* Explicit worker intercepts/slopes and firm dummies identify an independent
* likelihood and sandwich reference. The absorbed constant's normalization is
* arbitrary, so compare the full x/z covariance block and all fitted values.
quietly poisson count x z i.worker##c.period i.firm, vce(robust) ///
    tolerance(1e-12) ltolerance(1e-12) nrtolerance(1e-12) iterate(1000) nolog
assert e(converged) == 1 & e(N) == 600
assert e(sample) == 1
tempname reference_b reference_V reference_se
matrix `reference_b' = (_b[x], _b[z])
matrix `reference_V' = e(V)[1..2,1..2]
matrix `reference_se' = (_se[x], _se[z])
predict double reference_eta, xb
predict double reference_mu, n
assert reference_eta < . & reference_mu > 0 & reference_mu < .

* These prospective bounds match the independent dummy-model witnesses:
* coefficients 1e-7 absolute; the entire covariance/SE block 1e-10 + 1e-6*|ref|;
* eta and mu 1e-7*(1+|ref|). No failed certificate may be accepted as a fit.
foreach workers in 0 1 4 {
    quietly feppml count x z, absorb(worker##c.period firm) ///
        threads(`workers') eta(actual_eta) mu(actual_mu)
    assert e(converged) == 1 & e(N) == 600
    assert e(sample) == 1
    assert e(separated) == 0 & e(num_singletons) == 0
    assert `"`e(vce)'"' == "robust"
    tempname actual_b actual_V actual_se db dv dse
    matrix `actual_b' = (_b[x], _b[z])
    matrix `actual_V' = e(V)[1..2,1..2]
    matrix `actual_se' = (_se[x], _se[z])
    mata: st_numscalar("`db'", max(abs(st_matrix("`actual_b'")-st_matrix("`reference_b'"))))
    mata: st_numscalar("`dv'", max(abs(st_matrix("`actual_V'")-st_matrix("`reference_V'")) :/ (1e-10 :+ 1e-6:*abs(st_matrix("`reference_V'")))))
    mata: st_numscalar("`dse'", max(abs(st_matrix("`actual_se'")-st_matrix("`reference_se'")) :/ (1e-10 :+ 1e-6:*abs(st_matrix("`reference_se'")))))
    assert actual_eta < . & actual_mu > 0 & actual_mu < .
    generate double eta_error = abs(actual_eta-reference_eta)/(1+abs(reference_eta))
    generate double mu_error = abs(actual_mu-reference_mu)/(1+abs(reference_mu))
    quietly summarize eta_error, meanonly
    local eta_error = r(max)
    quietly summarize mu_error, meanonly
    local mu_error = r(max)
    display as result "ORDINARY_SLOPE_ORACLE|workers=`workers'|b_max=" ///
        %24.17e scalar(`db') "|V_bound_ratio=" %24.17e scalar(`dv') ///
        "|SE_bound_ratio=" %24.17e scalar(`dse') ///
        "|eta_scaled_max=" %24.17e `eta_error' "|mu_scaled_max=" %24.17e `mu_error'
    assert scalar(`db') <= 1e-7
    assert scalar(`dv') <= 1 & scalar(`dse') <= 1
    assert `eta_error' <= 1e-7 & `mu_error' <= 1e-7
    drop actual_eta actual_mu eta_error mu_error
}

display as result "END_FIXTURE|stability061_ppml_ordinary_slope|rc=0"
