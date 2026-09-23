version 18.0
clear all
set more off

set obs 4096
generate long obs_id = _n
generate int g = 1 + mod(_n - 1, 32)
generate int h = 1 + mod(floor((_n - 1) / 32), 16)
generate double z = sin(_n / 23) + mod(_n, 5) / 7
generate double w = cos(_n / 19) - mod(_n, 3) / 6
generate double x = sin(_n / 17) + mod(_n, 7) / 5
generate double x2 = cos(_n / 29) - mod(_n, 11) / 9
generate double off = 0.1 * sin(_n / 31)
generate double y = cond(mod(_n, 11) == 0, 0, 1 + mod(_n, 19))
generate double pw = 0.2 + mod(_n, 13) / 3

foreach threads in 1 4 {
    * The production engine must be the measured engine. Warm/cold equivalence
    * is outside the 16-iteration window; allocate/free churn inside it fails.
    _fereg_estimate_ppml y x x2, pweight(pw) absorb(g##c.z h##c.w) offset(off) ///
        vce(robust) separation(none) keepsingletons threads(`threads') ///
        solver(map) memory(fast) allocationaudit ///
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
    * ADR 0016: xb includes the offset.
    assert abs(eta_audit - xb_audit - d_audit) <= 1e-10

    feppml y x x2 [pw=pw], absorb(g##c.z h##c.w) offset(off) vce(robust) ///
        separation(none) keepsingletons threads(`threads') mu(mu_public)
    assert mreldif(e(b), b_audit) <= 1e-8
    assert mreldif(e(V), V_audit) <= 1e-8
    assert abs(mu_public - mu_audit) <= 1e-8 * (1 + abs(mu_public))
    drop mu_audit eta_audit xb_audit d_audit mu_public
}

display as result "END_FIXTURE|feppml_joint_slope_audit|rc=0"
