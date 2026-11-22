version 18
clear all
set more off

* The streamed exact separation lanes must cover the design family, not only
* binary intercept designs of one shape. Each design below hides one cross cell
* of two ordered four-level factors that holds only zero outcomes, so the
* separated set is exactly that cell; every other pattern has positives. The
* variants add a continuous regressor or 70 indicator regressors; write
* absorbed controls as indicator regressors; take a design of a
* few thousand rows with a continuous regressor; and take the binary design at
* a row count where an earlier streamed lane refused its own row-proportional
* workspace. Near-finite twins place one positive inside the cell: the maximum
* likelihood estimate then exists and no row may be deleted.

capture program drop fixture_design
program define fixture_design
    args n seed
    clear
    set seed `seed'
    quietly set obs `n'
    generate long id = _n
    generate byte a = runiformint(0,3)
    generate byte b = cond(a>=2, runiformint(0,3), runiformint(0,1))
    generate byte hidden = a>=2 & b<2
    forvalues k = 1/28 {
        quietly generate byte f`k' = runiformint(0,1+mod(`k',4))
    }
    generate byte x = runiform()<.3
    generate int g = runiformint(1,400)
    generate byte y = runiform()<.006 & !hidden
    generate double z = rnormal()
    generate int w = runiformint(1,70)
end

capture program drop ladder_design
program define ladder_design
    args n seed rate
    clear
    set seed `seed'
    quietly set obs `n'
    generate long id = _n
    generate byte a = runiformint(0,3)
    generate byte hidden = a>=2 & runiform()<0.16
    generate byte b = cond(a<2 | hidden, runiformint(0,1), runiformint(2,3))
    forvalues k = 1/28 {
        quietly generate byte f`k' = runiformint(0,1+mod(`k',7))
    }
    generate byte x = runiform()<.3
    generate double z = rnormal()
    generate int g = runiformint(1,400)
    generate byte y = runiform()<`rate' & !hidden
end

capture program drop exact_cell
program define exact_cell
    args h seconds limit
    assert `seconds'<`limit'
    assert e(converged)==1 & e(num_singletons)==0
    assert e(separated)==`h'
    assert e(sample)==!hidden
    assert inlist(reason,10,11) if hidden
    assert reason==0 if !hidden
end

capture program drop timed_fit
program define timed_fit, rclass
    timer clear 1
    timer on 1
    quietly feppml `0'
    timer off 1
    quietly timer list 1
    return scalar seconds = r(t1)
end

capture program drop near_finite
program define near_finite
    quietly summarize id if hidden, meanonly
    quietly replace y = 1 if id==r(min)
end

local controls f1 f2 f3 f4 f5 f6 f7 f8 f9 f10 f11 f12 f13 f14 f15 f16 f17 f18 f19 f20 f21 f22 f23 f24 f25 f26 f27 f28
local absorb f1 f2 f3 a `controls' b

* 1. Continuous regressor: the binary lane declines and the valued lane on the
* raw reader must certify the deletion and the terminal sample.
fixture_design 72000 140172
quietly count if hidden
local h = r(N)
assert `h'>15000
timed_fit y x z, absorb(`absorb') vce(cluster g) dropreason(reason)
exact_cell `h' `=r(seconds)' 120
local b = _b[z]
local se = _se[z]
quietly feppml y x z if !hidden, absorb(`absorb') vce(cluster g) separation(none)
assert reldif(_b[z],`b')<1e-9 & reldif(_se[z],`se')<1e-9
drop reason

* 2. Seventy indicator regressors.
timed_fit y x i.w, absorb(`absorb') vce(cluster g) dropreason(reason)
exact_cell `h' `=r(seconds)' 120
local b = _b[x]
quietly feppml y x i.w if !hidden, absorb(`absorb') vce(cluster g) separation(none)
assert reldif(_b[x],`b')<1e-9
drop reason

* 3. The same model with the controls as indicator regressors.
timed_fit y x, absorb(`absorb') vce(cluster g) dropreason(reason)
exact_cell `h' `=r(seconds)' 120
local b = _b[x]
local se = _se[x]
drop reason
timed_fit y x i.(`controls'), absorb(a b) vce(cluster g) dropreason(reason)
exact_cell `h' `=r(seconds)' 120
assert reldif(_b[x],`b')<1e-9 & reldif(_se[x],`se')<1e-9
drop reason

* A near-finite twin of the continuous variant keeps every row.
near_finite
quietly feppml y x z, absorb(`absorb') vce(cluster g) dropreason(reason)
assert e(converged)==1 & e(separated)==0 & e(N)==_N & reason==0
drop reason

* 4. A few thousand rows with a continuous regressor.
ladder_design 6000 7438 .02
quietly count if hidden & y==0
local h = r(N)
timed_fit y x z, absorb(`controls' a b) vce(cluster g) dropreason(reason)
exact_cell `h' `=r(seconds)' 60
drop reason

* 5. The binary design at 480,000 rows: every streamed workspace, including
* the row-proportional part of the projected cone, is requested by size.
ladder_design 480000 481438 .006
quietly count if hidden & y==0
local h = r(N)
timed_fit y x, absorb(`controls' a b) vce(cluster g) dropreason(reason)
exact_cell `h' `=r(seconds)' 300
drop reason
near_finite
quietly feppml y x, absorb(`controls' a b) vce(cluster g) dropreason(reason)
assert e(converged)==1 & e(separated)==0 & e(N)==_N & reason==0

* 6. A separating direction with non-integer coefficients: w equals a level
* effect of h plus a positive shift on a tenth of the zero rows, so w minus the
* h effects separates exactly those rows. Its exact integer lift fails and the
* IR direction certificate must accept the deletion.
clear
set seed 2611
quietly set obs 20000
generate long id = _n
local small
forvalues k = 1/6 {
    quietly generate byte f`k' = runiformint(0,1+mod(`k',5))
    local small `small' f`k'
}
generate int g = runiformint(1,300)
generate byte x = runiform()<.3
generate int h = runiformint(1,40)
generate double c = rnormal()
bysort h (id): replace c = c[1]
sort id
generate byte y = runiform()<.05
generate byte hidden = y==0 & runiform()<.1
generate double w = c + cond(hidden, .2+runiform(), 0)
quietly count if hidden
local nh = r(N)
timed_fit y x w, absorb(`small' h) vce(cluster g) dropreason(reason)
assert r(seconds)<120
assert e(converged)==1 & e(N)==_N-`nh' & e(sample)==!hidden & (reason!=0)==hidden
local b = _b[x]
quietly feppml y x w if !hidden, absorb(`small' h) vce(cluster g) separation(none)
assert reldif(_b[x],`b')<1e-9

display "END_FIXTURE|feppml_streamed_design_family|rc=0"
