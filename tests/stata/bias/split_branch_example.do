version 18.0
clear all
set more off
set obs 480
generate long worker=ceil(_n/12)
generate byte branch=1+mod(worker,2)
generate byte worker_group=ceil(worker/10)
generate byte firm_group=1+mod(_n+worker,6)
generate double x=sin(_n/7)
generate double events=exp(.2*x+.1*worker_group-.08*firm_group)*(.75+.1*mod(_n,7))
local rng `"`c(rngstate)'"'
forvalues b=1/2 {
    quietly feppml events x if branch==`b', absorb(worker_group firm_group) ///
        biascorrection(randomized) biasunit(worker) biasprobes(256) ///
        biasseed(739391) biasstream(`b') biasfe(branch`b')
    assert e(sample)==(branch==`b')
    assert e(bias_diagnostics)[5,1]==`b'
    assert branch`b'_2_raw==0 if firm_group==1 & e(sample)
    assert branch`b'_2_d==0 if firm_group==1 & e(sample)
    assert branch`b'_2_c==0 if firm_group==1 & e(sample)
    assert missing(branch`b'_1_bc) if !e(sample)
    matrix corrected_branch`b'=e(b_bc)
}
assert `"`c(rngstate)'"'==`"`rng'"'
* Full/A/B raw effects and increments already share firm_group=1 here.
* A caller with a different anchor must re-anchor every channel consistently.
* Construct external cross-branch moments only after matching their support.
display "END_FIXTURE|split_branch_example|rc=0"
