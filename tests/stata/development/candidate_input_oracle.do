version 18.0
clear all
set more off
set obs 65540
generate long row = _n
generate byte g = mod(_n,8)
generate byte cl = mod(_n,17)
generate double x1 = sin(_n/17)
generate float x2 = cos(_n/31)
generate double w = 1 + mod(_n,5)
generate double off = 0.01*mod(_n,3)
generate double exposure = exp(off)
generate double y = exp(0.2*x1 - 0.1*x2 + g/20 + off) + 0.1*mod(_n,7)
local codes ". .a .b .c .d .e .f .g .h .i .j .k .l .m .n .o .p .q .r .s .t .u .v .w .x .y .z"
forvalues j=1/27 {
    local code : word `j' of `codes'
    quietly replace x2 = `code' in `j'
    quietly replace off = `code' in `=`j'+27'
    quietly replace exposure = `code' in `=`j'+27'
}
quietly replace cl = .z in 60
generate str8 label = cond(missing(cl), "", string(cl))
// Include a 63-row run, a 64-row run, a long run crossing the bulk chunk
// boundary in adjustment loading, and short fragmented candidates.
foreach sample in full runs fragmented {
    generate byte eligible = 1
    if "`sample'" == "runs" quietly replace eligible = ///
        inrange(row,2,64) | inrange(row,66,129) | row >= 131
    if "`sample'" == "fragmented" quietly replace eligible = mod(row,3) == 0
    foreach weight in aw fw pw {
        quietly regress y x1 x2 i.g if eligible [`weight'=w], vce(cluster cl)
        matrix oracle_b = e(b)
        matrix oracle_v = e(V)
        generate byte expected = e(sample)
        quietly fereg y x1 x2 if eligible [`weight'=w], absorb(g) vce(cluster label)
        assert e(sample) == expected
        forvalues j=1/2 {
            assert abs(e(b)[1,`j'] - oracle_b[1,`j']) < 1e-9
            forvalues k=1/2 {
                assert abs(e(V)[`j',`k'] - oracle_v[`j',`k']) < 1e-10
            }
        }
        assert row == _n
        drop expected
    }
    quietly poisson y x1 x2 i.g if eligible [fw=w], offset(off) nolog
    matrix oracle_b = e(b)
    generate byte expected = e(sample)
    quietly feppml y x1 x2 if eligible [fw=w], absorb(g) exposure(exposure) mu(mu_exp)
    assert e(sample) == expected
    forvalues j=1/2 {
        assert abs(e(b)[1,`j'] - oracle_b[1,`j']) < 1e-7
    }
    matrix exp_b = e(b)
    matrix exp_v = e(V)
    quietly feppml y x1 x2 if eligible [fw=w], absorb(g) offset(off) mu(mu_off)
    assert e(sample) == expected
    assert mreldif(e(b),exp_b) < 1e-10
    assert mreldif(e(V),exp_v) < 1e-9
    assert reldif(mu_exp,mu_off) < 1e-10 if expected
    assert row == _n
    drop expected eligible mu_exp mu_off
}

// Errors must still name the first invalid original row, and excluded bad
// values may not reach either scientific classifier or adjustment loading.
quietly replace w = -1 in 65539
quietly replace exposure = 0 in 65540
quietly feppml y x1 x2 if row <= 65538 [pw=w], absorb(g) exposure(exposure)
assert !e(sample) in 65539/65540
matrix before_b = e(b)
generate byte before_sample = e(sample)
capture quietly feppml y x1 x2 [pw=w], absorb(g) exposure(exposure) mu(failed_mu)
assert _rc == 459
assert mreldif(e(b),before_b) == 0
assert e(sample) == before_sample
confirm new variable failed_mu
assert row == _n

display as result "END_FIXTURE|candidate_input_oracle|rc=0"
