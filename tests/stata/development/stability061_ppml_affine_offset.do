version 18.0
clear all
set more off

* A location change of an identified finite regressor cannot remove its
* within-group contrast. Every (g,h,z) pattern occurs at both x=-1 and x=1,
* with the positive-side outcome exactly twice the negative-side outcome.
* Thus the independently known PPML slope is log(2)/2 in every design below.
set obs 48
generate byte g = 1 + floor((_n - 1) / 12)
generate byte rep = mod(_n - 1, 6)
generate byte h = mod(rep, 3)
generate double z = rep / 5
generate double x0 = cond(mod(floor((_n - 1) / 6), 2), 1, -1)
generate double x = x0
generate double w = 1 + h
generate double y = (1 + g) * (1 + h) * (1 + mod(rep, 2)) * cond(x0 < 0, 1, 2)
generate double off = 0

foreach design in 1 2 3 {
    local absorb "g"
    if `design' == 2 local absorb "g h"
    if `design' == 3 local absorb "g##c.z h"
    foreach memory in fast low {
        quietly feppml y x0 [pw=w], absorb(`absorb') memory(`memory') threads(1) eta(reference_eta)
        assert abs(_b[x0] - ln(2)/2) <= 1e-9
        scalar reference_b = _b[x0]
        scalar reference_se = _se[x0]
        generate byte reference_sample = e(sample)
        foreach shift in 1000000 -100000000 100000000 {
            quietly replace x = x0 + `shift'
            quietly feppml y x [pw=w], absorb(`absorb') memory(`memory') threads(1) eta(shifted_eta)
            assert e(sample) == reference_sample
            assert abs(_b[x] - reference_b) <= 1e-7
            assert abs(_se[x] / reference_se - 1) <= 1e-6
            assert abs(shifted_eta - reference_eta) <= 1e-6
            drop shifted_eta
        }
        drop reference_eta reference_sample
    }
}

* A regressor component in an offset only translates its coefficient. A
* large starting offset must not strand an otherwise benign model at a
* singular initial Hessian or an unrepresentable initial Poisson mean.
quietly replace x = x0
foreach absorbed in 0 1 {
    local options ""
    if `absorbed' local options "absorb(g)"
    quietly feppml y x, `options' threads(1) eta(reference_eta)
    scalar reference_b = _b[x]
    scalar reference_se = _se[x]
    foreach shift in 20 100 500 {
        quietly replace off = `shift' * x
        quietly feppml y x, `options' offset(off) threads(1) eta(shifted_eta)
        assert abs(_b[x] + `shift' - reference_b) <= 1e-8
        assert abs(_se[x] / reference_se - 1) <= 1e-7
        assert abs(shifted_eta - reference_eta) <= 1e-8
        drop shifted_eta
    }
    drop reference_eta
}

* Exact separator -x + 10^15: -1 on every zero-outcome row, zero on every
* positive row. All values and the contrast are exactly representable.
* Checking only raw-column-normalized moments can miss this contrast.
clear
set obs 48
generate byte g = 1 + floor((_n - 1) / 12)
generate double x0 = mod(_n, 2)
generate double y = cond(x0, 0, 1 + mod(_n, 3))
generate double x = x0 + 1e15
quietly feppml y x0, absorb(g) threads(1)
assert e(N) == 24
generate byte reference_sample = e(sample)
quietly feppml y x, absorb(g) threads(1)
assert e(N) == 24
assert e(sample) == reference_sample
assert !e(sample) if y == 0

* The same separating direction also belongs to absorbed intercept-plus-slope
* channels; a full-span dual certificate must retain their centered contrast.
quietly feppml y, absorb(g##c.x0) threads(1)
assert e(N) == 24
assert e(sample) == reference_sample
quietly feppml y, absorb(g##c.x) threads(1)
assert e(N) == 24
assert e(sample) == reference_sample

display as result "END_FIXTURE|stability061_ppml_affine_offset|rc=0"
