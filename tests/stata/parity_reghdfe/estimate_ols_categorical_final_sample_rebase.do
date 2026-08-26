version 18.0
clear
set more off

set obs 10
generate long obs_id = _n
generate byte a = real(word("1 2 2 2 2 2 2 2 2 3", _n))
generate byte b = real(word("1 1 1 1 1 2 2 2 2 3", _n))
generate byte f = real(word("1 2 2 3 3 2 2 3 3 1", _n))
generate byte z = real(word("1 1 2 1 2 1 2 1 2 2", _n))
generate double x = real(word("-1.5 -1.0 -0.4 0.2 0.9 -0.8 -0.1 0.5 1.2 1.7", _n))
generate double y = 1.4*x + 0.7*(f == 3) - 0.3*(z == 2) + ///
    0.2*(f == 3)*(z == 2) + 0.1*a - 0.2*b

_fereg_estimate_ols_test y c.x i.f, absorb(a#b) residual(rebase_resid)
assert e(N) == 8
assert e(N_full) == 10
assert e(num_singletons) == 2
assert e(fereg_receipt)[1,5] == 4
assert `"`e(fereg_result_wire)'"' == "1.4"
local names : colfullnames e(b)
assert `"`names'"' == "x 2b.f 3.f _cons"
assert e(fereg_status)[1,2] == 1
assert e(fereg_status)[2,2] == 2
assert e(fereg_status)[3,2] == 1
assert e(fereg_status)[4,2] == 5
assert e(sample) == inrange(obs_id, 2, 9)
assert missing(rebase_resid) if !e(sample)

matrix __frg_expected_b = ( ///
    +1.1c5bec29a16c4X+000, 0, +1.2b33c240ae588X+000, -1.aa4b4c5891d8bX-002)
matrix __frg_expected_V = ( ///
    +1.a1a4bd17208efX-008, 0, -1.0a3f6bbebe5b1X-007, +1.e04a3fda98a45X-009 \ ///
    0, 0, 0, 0 \ ///
    -1.0a3f6bbebe5b1X-007, 0, +1.b321e0874bae8X-007, -1.91d9f30f73e31X-008 \ ///
    +1.e04a3fda98a45X-009, 0, -1.91d9f30f73e31X-008, +1.d37fec9f7cecdX-009)
forvalues row = 1/4 {
    assert abs(e(b)[1,`row'] - __frg_expected_b[1,`row']) <= ///
        5e-9 * max(abs(__frg_expected_b[1,`row']), 1)
    forvalues column = 1/4 {
        assert abs(e(V)[`row',`column'] - __frg_expected_V[`row',`column']) <= ///
            5e-9 * max(abs(__frg_expected_V[`row',`column']), 1)
    }
}

_fereg_estimate_ols_test y c.x i.f##i.z, absorb(a#b)
assert e(N) == 8
assert e(N_full) == 10
assert e(num_singletons) == 2
assert e(fereg_receipt)[1,5] == 10
local names : colfullnames e(b)
assert `"`names'"' == ///
    "x 2b.f 3.f 1b.z 2.z 2b.f#1b.z 2b.f#2o.z 3o.f#1b.z 3.f#2.z _cons"
matrix __frg_expected_b = ( ///
    +1.666666666666cX+000, 0, +1.6666666666658X-001, 0, ///
    -1.3333333333346X-002, 0, 0, 0, +1.999999999999bX-003, ///
    -1.999999999993dX-004)
forvalues column = 1/10 {
    assert abs(e(b)[1,`column'] - __frg_expected_b[1,`column']) <= ///
        5e-8 * max(abs(__frg_expected_b[1,`column']), 1)
}
assert e(fereg_status)[1,2] == 1
assert e(fereg_status)[2,2] == 2
assert e(fereg_status)[3,2] == 1
assert e(fereg_status)[4,2] == 2
assert e(fereg_status)[5,2] == 1
assert e(fereg_status)[6,2] == 2
assert e(fereg_status)[7,2] == 2
assert e(fereg_status)[8,2] == 2
assert e(fereg_status)[9,2] == 1
assert e(fereg_status)[10,2] == 5
assert abs(e(V)[1,1] - +1.5b4db6db6daafX-05e) <= 5e-8
assert abs(e(V)[3,3] - +1.0ffb6b851eaefX-05d) <= 5e-8
assert abs(e(V)[5,5] - +1.2813880bb3e43X-05f) <= 5e-8
assert abs(e(V)[9,9] - +1.bc8c6f2d593a2X-065) <= 5e-8
assert abs(e(V)[10,10] - +1.19f79304636fbX-05e) <= 5e-8

tempname accepted_b accepted_V
matrix `accepted_b' = e(b)
matrix `accepted_V' = e(V)
foreach specification in "ib1.f" "ib3.f" "ibn.f" {
    capture noisily _fereg_estimate_ols_test y c.x `specification', absorb(a#b)
    assert _rc == 498
    assert mreldif(e(b), `accepted_b') == 0
    assert mreldif(e(V), `accepted_V') == 0
}

fvset base 3 f
capture noisily _fereg_estimate_ols_test y c.x i.f, absorb(a#b)
assert _rc == 498
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0
fvset clear f

capture noisily fereg y x, absorb(a#b)
assert _rc == 0
assert `"`e(cmd)'"' == "fereg"
assert e(N) == 8
capture noisily feppml y x, absorb(a#b)
assert _rc == 498

display "END_FIXTURE|estimate_ols_categorical_final_sample_rebase|rc=0"
