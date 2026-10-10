version 18.0
clear all
set more off
set seed 20260907
set obs 1200
generate long g1 = mod(_n-1,20)
generate long g2 = mod(floor((_n-1)/20),15)
generate long g3 = mod(floor((_n-1)/300),4)
generate double x1 = rnormal()
generate double x2 = rnormal()
generate double off = 0.05*rnormal()
generate double w = 1+mod(_n,3)
generate double y = 1+rpoisson(exp(0.2*x1-0.1*x2+g1/50+g2/60+off))

foreach effects in 1 2 3 {
    local absorb g1
    if `effects' >= 2 local absorb `absorb' g2
    if `effects' == 3 local absorb `absorb' g3
    local explicit map
    foreach k in 0 2 {
        local controls
        if `k' == 2 local controls x1 x2
        foreach t in 1 7 {
            quietly feppml y `controls' [fw=w], absorb(`absorb') offset(off) ///
                vce(robust) separation(none) solver(`explicit') threads(`t') mu(explicit_mu)
            matrix explicit_b = e(b)
            matrix explicit_v = e(V)
            scalar explicit_ll = e(ll)
            scalar explicit_dev = e(deviance)
            scalar explicit_rank = e(rank)
            scalar explicit_df = e(df_a)
            local engine `e(feppml_solver_engine)'
            local checkpoint `e(feppml_solver_checkpoint)'
            generate byte explicit_sample = e(sample)
            quietly feppml y `controls' [fw=w], absorb(`absorb') offset(off) ///
                vce(robust) separation(none) solver(auto) threads(`t') mu(auto_mu)
            assert `"`e(feppml_solver_requested)'"' == "auto"
            assert `"`e(feppml_solver_selected)'"' == "`explicit'"
            assert `"`e(feppml_solver_effective)'"' == "`explicit'"
            assert `"`e(feppml_solver_engine)'"' == "`engine'"
            assert `"`e(feppml_solver_checkpoint)'"' == "`checkpoint'"
            assert e(feppml_solver_fallback_count) == 0
            assert e(feppml_solver)[1,1] == 1
            assert mreldif(e(b),explicit_b) == 0
            assert mreldif(e(V),explicit_v) == 0
            assert e(ll) == explicit_ll
            assert e(deviance) == explicit_dev
            assert e(rank) == explicit_rank
            assert e(df_a) == explicit_df
            assert e(sample) == explicit_sample
            assert auto_mu == explicit_mu
            drop auto_mu explicit_mu explicit_sample
        }
    }
}
quietly feppml y x1 x2, absorb(g1 g2) separation(none) memory(low) solver(auto)
assert `"`e(feppml_solver_selected)'"' == "map"
assert e(feppml_solver_fallback_count) == 0
quietly feppml y x1 x2, absorb(g1 g2) separation(none) memory(fast) solver(auto)
assert `"`e(feppml_solver_selected)'"' == "map"
estimates store before_failed_auto
matrix before_b = e(b)
capture noisily feppml y x1 x2, absorb(g1 g2) separation(none) solver(auto) iterate(1)
assert _rc == 430
assert mreldif(e(b),before_b) == 0
assert `"`e(feppml_solver_selected)'"' == "map"
estimates drop before_failed_auto

quietly feppml y x1 x2 [fw=w], absorb(exp_fe1=g1 exp_fe2=g2 exp_fe3=g3) ///
    offset(off) vce(robust) separation(none) solver(map) threads(7) d(exp_saved_d)
predict double exp_eta, eta
predict double exp_d, d
quietly feppml y x1 x2 [fw=w], absorb(auto_fe1=g1 auto_fe2=g2 auto_fe3=g3) ///
    offset(off) vce(robust) separation(none) solver(auto) threads(7) d(auto_saved_d)
predict double auto_eta, eta
predict double auto_d, d
assert exp_eta == auto_eta
assert exp_d == auto_d
forvalues j = 1/3 {
    assert exp_fe`j' == auto_fe`j'
}
assert abs(auto_d-auto_fe1-auto_fe2-auto_fe3) <= 1e-8
display as result "END_FIXTURE|feppml_auto_route|rc=0"
