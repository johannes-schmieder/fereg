version 18.0
clear all
set more off
set obs 81
generate byte g = cond(_n == 81, 9, mod(_n, 4))
generate byte f = cond(_n == 81, 9, mod(_n, 3))
generate double z = cond(_n == 81, 2, 1 + mod(_n, 7)/5)
generate double x = sin(_n/3) + mod(_n, 11)/10
generate double y = 2*x + z*(1+g) + 0.1*cos(_n)
generate byte w = cond(_n == 81, 2, 1)

* A retained physical singleton has multiplicity two. Its factor indicator is
* exactly spanned by its group slope; the identified fit remains well defined.
quietly regress y x i.f i.g#c.z [fw=w], noconstant
scalar reference_b = _b[x]
scalar reference_v = _se[x]^2
scalar reference_df = e(df_r)
predict double reference_resid, residuals
foreach memory in fast low {
    quietly fereg y x i.f [fw=w], absorb(g#c.z) ///
        memory(`memory') residuals(actual_resid) threads(1) deterministic
    assert abs(_b[x] - reference_b) < 1e-10
    assert abs(_se[x]^2 - reference_v) < 1e-10
    assert abs(actual_resid - reference_resid) < 1e-8
    assert e(N) == 82 & e(num_singletons) == 0 & e(df_r) == reference_df
    assert e(sample) == 1
    drop actual_resid
}

* Unit multiplicity is pruned unless the user explicitly keeps singletons.
replace w = 1 in 81
foreach keep in "" "keepsingletons" {
    local last = cond("`keep'" == "", 80, 81)
    quietly regress y x i.f i.g#c.z [fw=w] in 1/`last', noconstant
    scalar reference_b = _b[x]
    predict double unit_resid if e(sample), residuals
    foreach memory in fast low {
        quietly fereg y x i.f [fw=w], absorb(g#c.z) ///
            memory(`memory') `keep' residuals(actual_resid) threads(1) deterministic
        assert abs(_b[x] - reference_b) < 1e-10
        assert abs(actual_resid - unit_resid) < 1e-8 if e(sample)
        assert e(N) == `last'
        assert e(sample) == (_n <= `last')
        drop actual_resid
    }
    drop unit_resid
}
di as result "END_FIXTURE|frequency_singleton_validation|rc=0"
