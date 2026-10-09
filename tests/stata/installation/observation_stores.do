version 18.0
clear all
set more off

* A complete sample above 4,096 rows reaches the OLS batched store path.
set obs 8192
generate double x = mod(7 * _n, 29) / 14 - 1
generate double y = 0.7 + mod(11 * _n, 23) / 5
generate byte g = mod(_n, 8)

quietly regress y c.x i.g
predict double oracle_resid, residuals
predict double oracle_pred, xb

fereg y c.x, absorb(saved_g=g) residuals(ols_resid) threads(2) deterministic
assert e(N) == 8192
assert e(sample) == 1
predict double ols_pred, xbd
assert !missing(ols_resid, ols_pred, saved_g)
assert abs(ols_resid - oracle_resid) < 1e-10
assert abs(ols_pred - oracle_pred) < 1e-10
assert abs(y - ols_resid - ols_pred) < 1e-10

feppml y c.x, absorb(g) residuals(ppml_resid) mu(ppml_mu) ///
    xb(ppml_xb) eta(ppml_eta) d(ppml_d) threads(2) deterministic
assert e(N) == 8192
assert e(sample) == 1
assert !missing(ppml_resid, ppml_mu, ppml_xb, ppml_eta, ppml_d)
assert abs(y - ppml_resid - ppml_mu) < 1e-10
assert abs(ppml_eta - ppml_xb - ppml_d) < 1e-10
assert abs(exp(ppml_eta) - ppml_mu) < 1e-10
predict double ppml_pred, mu
assert abs(ppml_pred - ppml_mu) < 1e-10

display as result "END_FIXTURE|observation_stores|rc=0"
