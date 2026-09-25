version 18.0
clear all
set more off

* Repeated sparse triples make compact coupling affordable, while the 768
* remaining declared FE coordinates exclude the dense Schur bank by work.
* A fixed process/configuration keeps the real warmed allocation audit strict.
set obs 24576
generate long obs_id = _n
generate long cell = mod(_n - 1, 3072)
generate int g = 1 + mod(cell, 192)
generate int h = 1 + mod(37 * cell + floor(cell / 192), 256)
generate int j = 1 + mod(19 * cell + floor(cell / 64), 256)
generate double z2 = sin(_n / 37) + mod(_n, 11) / 9
generate double z = sin(_n / 23) + mod(_n, 5) / 7
generate double w = cos(_n / 19) - mod(_n, 3) / 6
generate double x = sin(_n / 17) + mod(_n, 7) / 5
generate double x2 = cos(_n / 29) - mod(_n, 11) / 9
generate double off = 0.1 * sin(_n / 31)
generate double y = cond(mod(_n, 11) == 0, 0, 1 + mod(_n, 19))
generate double pw = 0.2 + mod(_n, 13) / 3

* Run one fixed configuration per process. Crossbeam may reclaim retired
* thread-pool queues during a later action; the strict allocation window must
* still report exactly zero events and zero current growth.
local memory : environment FEREG_TEST_AUDIT_MEMORY
if "`memory'" == "" local memory fast
assert inlist("`memory'", "fast", "low")
local poolsize : environment FEREG_TEST_AUDIT_POOLSIZE
local threads : environment FEREG_TEST_AUDIT_THREADS
if "`poolsize'" == "" local poolsize 2
if "`threads'" == "" local threads 1
assert inlist(real("`poolsize'"), 1, 2)
assert inlist(real("`threads'"), 1, 4)
    * The production engine must be the measured engine. Warm/cold equivalence
    * is outside the 16-iteration window; allocate/free churn inside it fails.
    _fereg_estimate_ppml y x x2, pweight(pw) absorb(g##c.(z z2) h##c.w j) offset(off) ///
        vce(robust) separation(none) keepsingletons threads(`threads') ///
        solver(map) memory(`memory') poolsize(`poolsize') allocationaudit ///
        mu(mu_audit) eta(eta_audit) xb(xb_audit) d(d_audit)
    assert e(converged) == 1
    assert e(feppml_audit_iterations) == 16
    assert e(feppml_audit_events_delta) == 0
    assert e(feppml_audit_current_growth) == 0
    assert e(feppml_ppml_capacity_stable) == 1
    assert e(feppml_linear_capacity_stable) == 1
    assert e(feppml_fresh_warm_equivalent) == 1
    assert e(feppml_allocation_cert_passed) == 1
    matrix b_audit = e(b)
    matrix V_audit = e(V)
    * Compare the audited engine with the ordinary fast public fit.
    * ADR 0016: xb includes the offset.
    assert abs(eta_audit - xb_audit - d_audit) <= 1e-10

    feppml y x x2 [pw=pw], absorb(g##c.(z z2) h##c.w j) offset(off) vce(robust) ///
        separation(none) keepsingletons threads(`threads') mu(mu_public)
    assert mreldif(e(b), b_audit) <= 1e-8
    assert mreldif(e(V), V_audit) <= 1e-8
    assert abs(mu_public - mu_audit) <= 1e-8 * (1 + abs(mu_public))
    drop mu_audit eta_audit xb_audit d_audit mu_public

display as result "END_FIXTURE|feppml_compact_joint_slope_audit|rc=0"
