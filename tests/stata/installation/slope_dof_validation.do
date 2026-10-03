version 18.0
clear all
set more off
set obs 80
generate byte g = _n > 40
generate double z = cond(g, mod(_n, 7) / 5, 1)
generate double x = sin(_n / 3) + mod(_n, 11) / 10
generate double y = 2*x + 3*g + z*(1+g) + 0.1*cos(_n)
quietly regress y x i.g##c.z
scalar reference_b = _b[x]
scalar reference_v = _se[x]^2
scalar reference_df = e(df_r)
predict double reference_resid, residuals

capture noisily fereg y x, absorb(g##c.z) dof(continuous) engine(legacy)
assert _rc == 498
foreach memory in fast low {
    quietly fereg y x, absorb(g##c.z) dof(continuous) engine(next) ///
        memory(`memory') residuals(actual_resid) threads(1) deterministic
    assert abs(_b[x] - reference_b) < 1e-10
    assert abs(_se[x]^2 - reference_v) < 1e-10
    assert abs(actual_resid - reference_resid) < 1e-8
    assert e(df_a) == 3 & e(df_r) == reference_df
    assert e(dof_table)[2,1] == 2 & e(dof_table)[2,2] == 1 & e(dof_table)[2,3] == 1
    drop actual_resid
}
di as result "END_FIXTURE|slope_dof_validation|rc=0"
