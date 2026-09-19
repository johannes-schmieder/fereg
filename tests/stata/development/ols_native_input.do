version 18.0
clear all
set more off
set seed 905831
set obs 540

// Exercise the production raw-double loader against the SPI missing callback
// path (if 1), independently checking Stata's sample and explicit FE dummies.
generate long row = _n
generate byte group = mod(_n, 9)
generate byte factor = mod(_n, 3)
generate double z = rnormal()
generate double y = 0.4*z + group/10 + rnormal()
generate double w = 1 + mod(_n, 7)
local codes ". .a .b .c .d .e .f .g .h .i .j .k .l .m .n .o .p .q .r .s .t .u .v .w .x .y .z"
foreach storage in byte int long float double {
    generate `storage' x = mod(_n, 83) - 40
    forvalues j=1/27 {
        local code : word `j' of `codes'
        quietly replace x = `code' in `j'
    }
    quietly replace y = .a in 31
    quietly replace group = .z in 32
    quietly replace w = .b in 33
    foreach weight in none aw fw pw {
        local wt
        if "`weight'" != "none" local wt [`weight'=w]
        generate byte expected = !missing(y,x,z,group)
        if "`weight'" != "none" quietly replace expected = 0 if missing(w)
        foreach source in y x z group w {
            clonevar original_`source' = `source'
        }
        quietly fereg y x z `wt', absorb(group) vce(robust) residuals(native_resid)
        foreach source in y x z group w {
            assert `source' == original_`source'
            drop original_`source'
        }
        assert row == _n
        assert e(sample) == expected
        matrix raw_b = e(b)
        matrix raw_v = e(V)
        matrix raw_status = e(fereg_status)
        scalar raw_rss = e(rss)
        scalar raw_df = e(df_r)
        quietly fereg y x z if 1 `wt', absorb(group) vce(robust) residuals(scalar_resid)
        assert row == _n
        assert e(sample) == expected
        assert mreldif(e(b),raw_b) == 0
        assert mreldif(e(V),raw_v) == 0
        assert mreldif(e(fereg_status),raw_status) == 0
        assert native_resid == scalar_resid
        quietly regress y x z i.group `wt', vce(robust)
        assert e(sample) == expected
        assert e(df_r) == raw_df
        assert abs(e(rss)-raw_rss) < 1e-10
        forvalues j=1/2 {
            assert abs(e(b)[1,`j']-raw_b[1,`j']) < 1e-10
            forvalues k=1/2 {
                assert abs(e(V)[`j',`k']-raw_v[`j',`k']) < 1e-10
            }
        }
        drop expected native_resid scalar_resid
    }
    drop x
}

// Huge nonmissing doubles are valid FE labels, and singleton deletion still
// occurs after native missing-value exclusion. No numeric sentinel is guessed.
generate double huge_group = cond(mod(_n,2),8e307,-8e307)
quietly replace huge_group = .z in 40
quietly replace huge_group = 0 in 41
quietly fereg y z, absorb(huge_group)
generate byte bulk_sample = e(sample)
assert !bulk_sample in 40/41
matrix bulk_b = e(b)
matrix bulk_v = e(V)
quietly fereg y z if 1, absorb(huge_group)
assert e(sample) == bulk_sample
assert mreldif(e(b),bulk_b) == 0
assert mreldif(e(V),bulk_v) == 0
assert row == _n

// An excluded invalid weight must never enter native domain checks.
quietly replace w = -1 in 1
quietly fereg y z if row > 1 [aw=w], absorb(group)
assert !e(sample) in 1
assert row == _n

// Ordinary factor expansion and stored sort metadata must not alter row order.
sort row
local sorted_before : sortedby
quietly fereg y z i.factor, absorb(group)
local sorted_after : sortedby
assert "`sorted_after'" == "`sorted_before'"
assert row == _n

// Cluster sources and fixed-width strings retain the same sample semantics.
generate str8 label = cond(missing(group), "", string(group))
foreach cl in group label {
    quietly fereg y z, absorb(group) vce(cluster `cl')
    matrix cluster_b = e(b)
    matrix cluster_v = e(V)
    generate byte cluster_sample = e(sample)
    quietly fereg y z if 1, absorb(group) vce(cluster `cl')
    assert e(sample) == cluster_sample
    assert mreldif(e(b),cluster_b) == 0
    assert mreldif(e(V),cluster_v) == 0
    drop cluster_sample
}

// Call the transaction directly so no outer sortpreserve wrapper can mask an
// order change on any of its injected failure exits.
sort group row
generate long entry_order = _n
generate byte entry_sample = e(sample)
matrix entry_b = e(b)
foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics precommit {
    capture quietly _fereg_estimate_ols y z, absorb(group) inject(`stage')
    assert _rc == 3498
    assert entry_order == _n
    assert entry_sample == e(sample)
    assert mreldif(e(b),entry_b) == 0
    local sorted_after : sortedby
    assert "`sorted_after'" == "group row"
}

// A separate dummy regression checks the wide IID path on every retained
// input, including the preparation threshold and requested worker counts.
clear
set obs 16416
generate long g = mod(_n,64)
forvalues j=1/50 {
    generate double x`j' = rnormal()
}
generate double y = x1/3 - x2/5 + g/20 + rnormal()
quietly regress y x1-x50 i.g
matrix oracle_b = e(b)
matrix oracle_v = e(V)
scalar oracle_rss = e(rss)
scalar oracle_df = e(df_r)
foreach t in 1 4 16 {
    quietly fereg y x1-x50, absorb(g) threads(`t')
    assert e(df_r) == oracle_df
    assert abs(e(rss)-oracle_rss) < 1e-9*oracle_rss
    forvalues j=1/50 {
        assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-9
        forvalues k=1/50 {
            assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
        }
    }
}

// Incomplete wide samples exercise native mask classification and compaction.
// regress supplies an independent dummy-variable oracle on each exact sample.
generate double original_x50 = x50
generate byte eligible = mod(_n, 17) == 0
foreach case in sparse dense restricted {
    quietly replace x50 = original_x50
    if "`case'" == "sparse" quietly replace x50 = .a if mod(_n,997) == 0
    if "`case'" == "dense" quietly replace x50 = .z if mod(_n,7) != 0
    local restriction
    if "`case'" == "restricted" local restriction if eligible
    quietly regress y x1-x50 i.g `restriction'
    matrix oracle_b = e(b)
    matrix oracle_v = e(V)
    scalar oracle_rss = e(rss)
    scalar oracle_df = e(df_r)
    generate byte oracle_sample = e(sample)
    foreach t in 1 4 16 {
        quietly fereg y x1-x50 `restriction', absorb(g) threads(`t')
        assert e(sample) == oracle_sample
        assert e(df_r) == oracle_df
        assert abs(e(rss)-oracle_rss) < 1e-9*oracle_rss
        forvalues j=1/50 {
            assert abs(e(b)[1,`j']-oracle_b[1,`j']) < 1e-9
            forvalues k=1/50 {
                assert abs(e(V)[`j',`k']-oracle_v[`j',`k']) < 1e-10
            }
        }
    }
    drop oracle_sample
}

display as result "END_FIXTURE|ols_native_input|rc=0"
