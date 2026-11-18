version 18.0
clear all
set more off

* Independent positive-mean model on two disconnected
* three-effect components. Saved channels must reconstruct the accepted fit
* without changing its coefficients, covariance, sample or aggregate predictor.
local workers = min(4, c(processors_mach), c(processors_lic))
local stress : environment FEREG_ORACLE_WORKERS
if `"`stress'"' != "" local workers = `stress'
set obs 1920
generate long row = _n - 1
generate byte component = floor(row / 960)
generate int worker = mod(floor(row / 80), 12)
generate byte period = mod(floor(row / 16), 5)
generate int firm = cond(period == 4, mod(worker + 1, 12), worker)
generate int g = component * 12 + worker
generate int h = component * 12 + firm
generate byte t = component * 5 + period
generate double x = sin(row * .37) + .2 * mod(row, 2)
generate double w = cond(period == 4, 1, .5 + mod(worker, 7))
generate double truth_eta = .2*x + .3 + .05*component + ///
    .1*sin(worker) + .035*cos(firm) + .01*period
generate double y = exp(truth_eta)

foreach tol in 1e-8 1e-12 {
    quietly feppml y x [pw=w], absorb(g h t) vce(robust) ///
        keepsingletons separation(none) threads(`workers') tolerance(`tol') ///
        d(base_d) eta(base_eta)
    assert abs(_b[x] - .2) <= 1e-9
    assert abs(base_eta - truth_eta) <= 1e-8 * (1 + abs(truth_eta))
    matrix base_b = e(b)
    matrix base_V = e(V)
    generate byte base_sample = e(sample)

    quietly feppml y x [pw=w], absorb(fe1=g fe2=h fe3=t) vce(robust) ///
        keepsingletons separation(none) threads(`workers') tolerance(`tol') ///
        d(saved_d) eta(saved_eta)
    assert mreldif(e(b), base_b) == 0
    assert mreldif(e(V), base_V) == 0
    assert e(sample) == base_sample
    assert saved_d == base_d if e(sample)
    assert saved_eta == base_eta if e(sample)
    assert abs(saved_d - fe1 - fe2 - fe3) <= `tol' * max(1, abs(saved_d)) if e(sample)
    forvalues j = 1/3 {
        clonevar repeat_fe`j' = fe`j'
    }
    drop fe1 fe2 fe3 saved_d saved_eta
    quietly feppml y x [pw=w], absorb(fe1=g fe2=h fe3=t) vce(robust) ///
        keepsingletons separation(none) threads(`workers') tolerance(`tol') ///
        d(saved_d) eta(saved_eta)
    assert mreldif(e(b), base_b) == 0
    assert mreldif(e(V), base_V) == 0
    assert e(sample) == base_sample
    assert saved_d == base_d if e(sample)
    assert saved_eta == base_eta if e(sample)
    forvalues j = 1/3 {
        assert fe`j' == repeat_fe`j' if e(sample)
    }
    drop base_d base_eta base_sample saved_d saved_eta fe1 fe2 fe3 repeat_fe*
}

* The exact finite fit has beta_u = beta_v = ln(2/3), means 1 and
* 2/3, and zero FE contribution. The accepted predictor contains only
* subtraction roundoff and can differ among rows of the same FE cell.
clear
input double (u v t y) byte retained
0  0  0  1  1
1  0  0  2  1
0  1  0  0  1
2 -1  0  0  1
0  0  1  0  0
2  1  1  0  0
end
expand 12
bysort u v t y: generate byte replicate = _n
generate byte g = mod(replicate, 3)
generate byte h = mod(floor((replicate - 1) / 3), 2)
generate long rid = _n
generate long perm = mod(rid * 29, 107)
generate double expected_mu = cond(u == 0 & v == 0, 1, 2.0 / 3) if retained

foreach order in rid perm {
    sort `order' rid
    foreach method in default simplex ir none {
        local restriction
        if "`method'" == "none" local restriction if retained
        quietly feppml y u v t `restriction', absorb(g h) separation(`method') ///
            vce(robust) threads(1) d(base_d) mu(base_mu)
        matrix base_b = e(b)
        matrix base_V = e(V)
        quietly feppml y u v t `restriction', absorb(fg=g fh=h) separation(`method') ///
            vce(robust) threads(1) d(saved_d) mu(saved_mu)
        assert mreldif(e(b), base_b) == 0
        assert mreldif(e(V), base_V) == 0
        assert e(sample) == retained
        assert saved_d == base_d & saved_mu == base_mu if retained
        assert abs(saved_mu - expected_mu) <= 1e-12 if retained
        assert abs(fg) <= 1e-12 & abs(fh) <= 1e-12 if retained
        assert abs(fg + fh - saved_d) <= 1e-12 if retained
        assert missing(fg) & missing(fh) if !retained
        drop base_d base_mu saved_d saved_mu fg fh
    }
}

display as result "END_FIXTURE|feppml_saved_intercept_recovery|rc=0"
