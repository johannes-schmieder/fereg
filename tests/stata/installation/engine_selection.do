version 18.0
clear all
set more off
macro drop FEREG_ENGINE
set obs 96
generate double x = mod(7 * _n, 29) / 14 - 1
generate double y = 0.7 + mod(11 * _n, 23) / 5
generate byte g = mod(_n, 8)

quietly fereg y c.x, absorb(g) threads(1) deterministic
assert "`e(fereg_engine)'" == "next"
matrix ols_b = e(b)
matrix ols_v = e(V)
generate byte ols_sample = e(sample)
estimates store next_ols
quietly feppml y c.x, absorb(g) threads(1) deterministic
assert "`e(feppml_engine)'" == "next"
matrix ppml_b = e(b)
matrix ppml_v = e(V)
generate byte ppml_sample = e(sample)

* Alternate both commands and both handles within the same Stata process.
foreach engine in legacy next legacy next {
    quietly fereg y c.x, absorb(g) threads(1) deterministic engine(`engine')
    assert "`e(fereg_engine)'" == "`engine'"
    assert mreldif(e(b), ols_b) == 0 & mreldif(e(V), ols_v) == 0
    assert e(sample) == ols_sample
    quietly feppml y c.x, absorb(g) threads(1) deterministic engine(`engine')
    assert "`e(feppml_engine)'" == "`engine'"
    assert mreldif(e(b), ppml_b) == 0 & mreldif(e(V), ppml_v) == 0
    assert e(sample) == ppml_sample
    foreach action in version doctor selftest {
        quietly fereg, `action' engine(`engine') threads(1)
        assert "`r(fereg_engine)'" == "`engine'"
        local legacy_name = strpos("`r(plugin_name)'", "fereg_legacy_") == 1
        assert `legacy_name' == ("`engine'" == "legacy")
    }
}

* Explicit selection outranks the global, including a malformed global.
global FEREG_ENGINE legacy
quietly fereg y c.x
assert "`e(fereg_engine)'" == "legacy"
quietly feppml y c.x
assert "`e(feppml_engine)'" == "legacy"
global FEREG_ENGINE malformed
foreach command in fereg feppml {
    capture noisily `command' y c.x
    assert _rc == 198
    quietly `command' y c.x, engine(next)
    capture noisily `command' y c.x, engine(malformed)
    assert _rc == 198
}
capture noisily fereg, version
assert _rc == 198
quietly fereg, version engine(next)
assert "`r(fereg_engine)'" == "next"
macro drop FEREG_ENGINE
estimates restore next_ols
assert "`e(fereg_engine)'" == "next"
assert mreldif(e(b), ols_b) == 0 & mreldif(e(V), ols_v) == 0
assert e(sample) == ols_sample

di as result "END_FIXTURE|engine_selection|rc=0"
