version 18
clear all
set more off
set obs 48
gen byte g=1+floor((_n-1)/12)
gen byte rep=mod(_n-1,6)
gen double x=cond(mod(floor((_n-1)/6),2),1,-1)
gen double y=(1+g)*cond(x<0,1+mod(rep,2),2+mod(rep,3))
gen double off=0

* Independent analytic cell means give beta=log(2)/2 and identical eta
* after subtracting any in-span offset coefficient. All outcomes are positive.
foreach absorbed in 0 1 {
    local a ""
    if `absorbed' local a "absorb(g)"
    quietly feppml y x, `a' threads(1) eta(eta0)
    assert abs(_b[x]-ln(2)/2)<2e-8
    scalar se0=_se[x]
    foreach mode in fast low {
        foreach shift in 1 5 10 20 100 500 {
            quietly replace off=`shift'*x
            quietly feppml y x, `a' offset(off) threads(1) memory(`mode') eta(etas)
            assert e(N)==48
            assert abs(_b[x]+`shift'-ln(2)/2)<2e-8
            assert abs(_se[x]/se0-1)<2e-8
            assert abs(etas-eta0)<2e-8
            drop etas
        }
    }
    drop eta0
}

* Preserve a genuine out-of-span offset. Built-in Poisson with explicit
* group indicators provides an independent dense model for this case.
generate double off_small=.2*(2*mod(rep,2)-1)
replace off=100*x+off_small
quietly poisson y x i.g, offset(off_small) nolog
scalar b_reference=_b[x]-100
predict double eta_reference, xb
quietly feppml y x, absorb(g) offset(off) threads(1) eta(eta_actual)
assert abs(_b[x]-b_reference)<2e-7
assert abs(eta_actual-eta_reference)<2e-7

display "END_FIXTURE|stability061_offset_start|rc=0"
