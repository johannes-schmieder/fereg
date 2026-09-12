version 18.0
clear all
set more off
set seed 982451653
set obs 540
generate long row = _n
generate byte g = mod(_n,9)
generate byte f = mod(_n,3)
generate double x = rnormal()
generate double y = 1 + mod(_n,7)
generate double w = 1 + mod(_n,5)
generate double off = 0.02*mod(_n,4)
generate double exposure = exp(off)
generate byte cl = mod(_n,17)

// The unrestricted native loader and candidate-gated SPI path must agree on
// every Stata missing code, including weights and adjustment sources.
local codes ". .a .b .c .d .e .f .g .h .i .j .k .l .m .n .o .p .q .r .s .t .u .v .w .x .y .z"
forvalues j=1/27 {
    local code : word `j' of `codes'
    quietly replace x = `code' in `j'
    quietly replace off = `code' in `=`j'+27'
    quietly replace exposure = `code' in `=`j'+27'
    quietly replace w = `code' in `=`j'+54'
}
quietly replace y = .a in 83
quietly replace g = .z in 84
quietly replace cl = .b in 85
// A cluster exclusion leaves an FE singleton, which must be pruned next.
quietly replace g = 99 in 85/86
quietly replace w = 1 in 86
generate str8 label = cond(missing(cl), "", string(cl))
sort row
foreach weight in none fw pw {
    local wt
    if "`weight'" != "none" local wt [`weight'=w]
    foreach adjustment in off exposure {
        local opt offset(off)
        if "`adjustment'" == "exposure" local opt exposure(exposure)
        foreach cluster in cl label {
            generate byte expected = !missing(y,x,g,cl,`adjustment') & g != 99
            if "`weight'" != "none" quietly replace expected = 0 if missing(w)
            quietly feppml y x `wt', absorb(g) vce(cluster `cluster') `opt' ///
                mu(native_mu) dropreason(native_drop)
            assert e(sample) == expected
            assert native_drop == 6 in 85
            assert native_drop == 8 in 86
            matrix native_b = e(b)
            matrix native_v = e(V)
            matrix native_status = e(feppml_status)
            quietly feppml y x if 1 `wt', absorb(g) vce(cluster `cluster') `opt' ///
                mu(scalar_mu) dropreason(scalar_drop)
            assert e(sample) == expected
            assert native_mu == scalar_mu
            assert native_drop == scalar_drop
            assert mreldif(e(b),native_b) == 0
            assert mreldif(e(V),native_v) == 0
            assert mreldif(e(feppml_status),native_status) == 0
            assert row == _n
            local sorted : sortedby
            assert "`sorted'" == "row"
            drop expected native_mu scalar_mu native_drop scalar_drop
        }
    }
}

// Metadata expansion, saved effects and native failure exits may not depend on
// an outer sortpreserve wrapper restoring observation order or sort metadata.
sort cl row
generate long entry_order = _n
quietly feppml y c.x i.f, absorb(saved_g=g) offset(off) mu(saved_mu)
assert entry_order == _n
local sorted : sortedby
assert "`sorted'" == "cl row"
drop saved_g saved_mu
matrix entry_b = e(b)
matrix entry_v = e(V)
generate byte entry_sample = e(sample)
foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics postpublic precommit {
    capture quietly _fereg_estimate_ppml y c.x i.f, absorb(failed_g=g) ///
        offset(off) mu(failed_mu) dropreason(failed_drop) inject(`stage')
    assert _rc == 3498
    assert entry_order == _n
    assert e(sample) == entry_sample
    assert mreldif(e(b),entry_b) == 0
    assert mreldif(e(V),entry_v) == 0
    local sorted : sortedby
    assert "`sorted'" == "cl row"
    confirm new variable failed_g
    confirm new variable failed_mu
    confirm new variable failed_drop
}

// Candidate-excluded invalid exposure and weights cannot cause domain errors;
// the same values on otherwise available candidate rows must fail atomically.
quietly replace exposure = 0 if row == 100
quietly replace w = -1 if row == 101
quietly feppml y x if !inlist(row,100,101) [pw=w], absorb(g) exposure(exposure)
assert !e(sample) if inlist(row,100,101)
drop entry_sample
generate byte entry_sample = e(sample)
matrix entry_b = e(b)
matrix entry_v = e(V)
capture quietly feppml y x [pw=w], absorb(g) exposure(exposure) mu(failed_mu)
assert _rc == 459
assert e(sample) == entry_sample
assert mreldif(e(b),entry_b) == 0
assert mreldif(e(V),entry_v) == 0
assert entry_order == _n
local sorted : sortedby
assert "`sorted'" == "cl row"
confirm new variable failed_mu

display as result "END_FIXTURE|feppml_native_preparation|rc=0"
