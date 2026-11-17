version 18.0
clear all
set more off

* Every (g,h,x) pattern has one zero and two positive outcomes. Consequently
* no full-model separating direction can be strict on any zero: an identical
* design row with positive y forces the direction to vanish there too.
* The exact cell mean is exp(.2*x+.05*g-.03*h)*(1.2+1.6)/3, which belongs to
* every equivalent crossed intercept/slope design below.
set obs 72
generate byte g = 1 + floor((_n - 1) / 18)
generate byte h = 1 + mod(floor((_n - 1) / 6), 3)
generate byte replicate = mod(_n - 1, 3)
generate double x = cond(mod(floor((_n - 1) / 3), 2), 1, -1)
generate double y = exp(.2*x + .05*g - .03*h) * cond(replicate == 0, 0, .8 + .4*replicate)
generate double reference_eta = .2*x + .05*g - .03*h + ln((1.2 + 1.6) / 3)
generate double x2 = x

foreach alias in same power integer affine split {
    quietly replace x2 = x
    local absorb "g##c.x h##c.x"
    if "`alias'" == "power" {
        quietly replace x2 = 2*x
        local absorb "g##c.x h##c.x2"
    }
    if "`alias'" == "integer" {
        quietly replace x2 = 3*x
        local absorb "g##c.x h##c.x2"
    }
    if "`alias'" == "affine" {
        quietly replace x2 = 8 + .5*x
        local absorb "g##c.x h##c.x2"
    }
    if "`alias'" == "split" local absorb "g h g#c.x h#c.x"
    quietly feppml y, absorb(`absorb') separation(ir) threads(1) eta(actual_eta)
    assert e(N) == 72
    assert e(sample)
    assert abs(actual_eta - reference_eta) < 1e-8
    drop actual_eta
}

* Exact additive alias on an unbalanced connected two-FE graph. Each edge
* has outcomes (0,2) on identical design rows, proving no separation and
* the finite MLE mu=1 (eta=0). Fixed-count coordinate sweeps cannot certify
* this alias; the complete source/membership identity must be established.
clear
input double (g h y)
1   1   0
1   1   2
1  10   0
1  10   2
2  10   0
2  10   2
2 100   0
2 100   2
3 100   0
3 100   2
end
generate double x_alias = g + h
quietly feppml y x_alias, absorb(g h) separation(ir) keepsingletons ///
    threads(1) eta(actual_eta) mu(actual_mean)
assert e(N) == 10
assert e(sample)
assert abs(actual_eta) < 1e-8
assert abs(actual_mean - 1) < 1e-8

display as result "END_FIXTURE|stability061_ppml_crossed_aliases|rc=0"
