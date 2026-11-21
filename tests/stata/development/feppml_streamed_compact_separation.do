version 18
clear all
set more off

* Two ordered four-level factors: b reaches its upper half only when a does.
* The cross cell (a high, b low) holds only zero outcomes, yet no single level
* of any factor is zero-only and the pair is neither the first absorbed pair
* nor centered on the largest effect. Every other pattern has positives, so
* the separated set is exactly that cell: the direction H(a)-H(b) is one on
* it and zero elsewhere. With 72,000 nearly distinct rows over 30 absorbed
* factors the packed compact simplex exceeds its fixed one-million-cell
* allowance; the streamed compact lane must certify the deletion and the
* separation-free terminal sample in bounded time. With outcomes as rare as
* in the motivating panels, fereg 0.6.7 instead runs its unit-metric
* recurrence without a certificate for its whole iteration budget.
set seed 140172
local n 72000
quietly set obs `n'
generate long id = _n
generate byte a = runiformint(0,3)
generate byte b = cond(a>=2, runiformint(0,3), runiformint(0,1))
generate byte hidden = a>=2 & b<2
local absorb
forvalues k = 1/28 {
    quietly generate byte f`k' = runiformint(0,1+mod(`k',4))
    local absorb `absorb' f`k'
}
local absorb f1 f2 f3 a `absorb' b
generate byte x = runiform()<.3
generate int g = runiformint(1,400)
generate byte y = runiform()<.006 & !hidden
quietly count if hidden
local h = r(N)
assert `h'>15000

timer clear 1
timer on 1
quietly feppml y x, absorb(`absorb') vce(cluster g) dropreason(reason)
timer off 1
quietly timer list 1
assert r(t1)<120
assert e(converged)==1 & e(num_singletons)==0
assert e(separated)==`h'
assert e(sample)==!hidden
assert reason==cond(hidden,10,0)
local b = _b[x]
local se = _se[x]
local N = e(N)

* The retained-sample fit without detectors must agree exactly in scope.
quietly feppml y x if !hidden, absorb(`absorb') vce(cluster g) separation(none)
assert e(N)==`N' & reldif(_b[x],`b')<1e-9 & reldif(_se[x],`se')<1e-9
drop reason

* Relabeled levels and reversed rows give the same sample and estimates.
preserve
replace a = 3-a
gsort -id
quietly feppml y x, absorb(`absorb') vce(cluster g) dropreason(reason)
assert e(separated)==`h' & e(sample)==!hidden & reason==cond(hidden,10,0)
assert reldif(_b[x],`b')<1e-9 & reldif(_se[x],`se')<1e-9
restore

* A requested IR without simplex reaches the same certified sample.
quietly feppml y x, absorb(`absorb') vce(cluster g) separation(fe ir) dropreason(reason)
assert e(separated)==`h' & e(sample)==!hidden & reason==cond(hidden,11,0)
assert reldif(_b[x],`b')<1e-9 & reldif(_se[x],`se')<1e-9
drop reason

* Near-finite control: one positive inside the cell removes the direction.
* The maximum likelihood estimate then exists and no row may be deleted.
quietly summarize id if hidden, meanonly
replace y = 1 if id==r(min)
quietly feppml y x, absorb(`absorb') vce(cluster g) dropreason(reason)
assert e(converged)==1 & e(separated)==0 & e(N)==_N & reason==0

display "END_FIXTURE|feppml_streamed_compact_separation|rc=0"
