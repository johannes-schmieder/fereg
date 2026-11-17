version 18.0
clear all
set more off

* Keep the full 65,536-row grouped shape: small or interleaved variants do
* not reproduce the strict unit-projection accumulation failure.
* x - 1[g=2] - 1[g=3] is -1 exactly on motif rows6/7, zero elsewhere.
input double(y x g h)
1 0 1 1
1 0 1 2
1 1 2 1
1 1 2 2
1 1 3 1
0 0 3 1
0 0 3 2
0 0 1 1
end
generate byte motif = _n
expand 8192
sort motif
assert _N == 65536
generate byte wanted_sample = !inlist(motif, 6, 7)

* Independent Poisson score solution after separation. For the retained
* two-by-two block, a=2-sqrt(2), b=c=2*(1-a), d=2*a. The third g cell has
* mean1 and remains in the sample because it has 8,192 members.
* The unit zero-indicator dual on motifs1,2,3,4,5,8 is
* (-3,-1,-1,1,0,4)/7: exact full-model moments0 and zero-outcome margin4/7.
tempname a
scalar `a' = 2 - sqrt(2)
generate double wanted_mu = .
replace wanted_mu = scalar(`a') if inlist(motif, 1, 8)
replace wanted_mu = 2*(1-scalar(`a')) if inlist(motif, 2, 3)
replace wanted_mu = 2*scalar(`a') if motif == 4
replace wanted_mu = 1 if motif == 5

foreach shape in separated control {
    local target_n 49152
    if "`shape'" == "control" {
        * Every identical design row has paired outcomes(0,2). Thus no zero
        * can be separated, and mu=1 is the exact finite MLE everywhere.
        replace y = 2*mod(_n-1, 2)
        replace wanted_sample = 1
        replace wanted_mu = 1
        local target_n 65536
    }
    foreach workers in 1 4 {
        quietly feppml y x, absorb(g h) vce(robust) separation(ir) ///
            threads(`workers') mu(actual_mu) eta(actual_eta)
        assert e(N) == `target_n'
        assert e(sample) == wanted_sample
        assert abs(actual_mu - wanted_mu) < 1e-8 if wanted_sample
        assert abs(actual_eta - ln(wanted_mu)) < 1e-8 if wanted_sample
        assert missing(actual_mu) & missing(actual_eta) if !wanted_sample
        drop actual_mu actual_eta
    }
}

display as result "END_FIXTURE|stability061_ppml_replicated_binary|rc=0"
