version 18.0
clear all
set more off
* Independent balanced model: the bounded MAP path must recover exact repeats
* without changing saved/unsaved fitted science, at both requested tolerances.
local workers = min(4, c(processors_mach), c(processors_lic))
set obs 240
generate byte g1 = mod(_n-1,4)
generate byte g2 = mod(floor((_n-1)/4),3)
generate byte g3 = mod(floor((_n-1)/12),2)
generate double x = sin(floor((_n-1)/24)*.7)
generate double truth = .2*x + .3 + .01*g1 + .02*g2 - .03*g3
generate double y = exp(truth)
foreach tol in 1e-8 1e-12 {
    quietly feppml y x, absorb(g1 g2 g3) separation(none) keepsingletons threads(`workers') tolerance(`tol') d(base) eta(eta0)
    matrix B = e(b)
    matrix V = e(V)
    quietly feppml y x, absorb(a=g1 b=g2 c=g3) separation(none) keepsingletons threads(`workers') tolerance(`tol') d(saved) eta(eta1)
    assert mreldif(B,e(b)) == 0
    assert mreldif(V,e(V)) == 0
    assert base == saved
    assert eta0 == eta1
    assert abs(eta1-truth) < 1e-10
    assert abs(saved-a-b-c) <= `tol'*max(1,abs(saved))
    clonevar repeat_a = a
    clonevar repeat_b = b
    clonevar repeat_c = c
    drop a b c saved eta1
    quietly feppml y x, absorb(a=g1 b=g2 c=g3) separation(none) keepsingletons threads(`workers') tolerance(`tol')
    assert a==repeat_a
    assert b==repeat_b
    assert c==repeat_c
    drop a b c base eta0 repeat_*
}
display as result "END_FIXTURE|feppml_saved_map_pilot|rc=0"
