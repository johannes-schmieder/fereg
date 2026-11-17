version 18.0
clear all
set more off
set obs 180
generate long t = _n
generate double x = sin(_n)
generate double z = cos(_n)
generate double y = 2 + 3*x + z + sin(1.7*_n)
generate double yp = exp(.1*x + .2*z) + mod(_n, 3)
generate byte f = mod(_n, 3)
generate byte g = mod(floor(_n/3), 3)
tsset t

* Stata canonicalizes o.x, co.x and o.c.x to the same explicit omitted
* continuous slot. Preserve that slot, including beside categorical
* interactions and in recognized omitted time-series spellings.
foreach rhs in "o.x z" "co.x z" "o.c.x z" ///
        "oL.x z" "L.o.x z" "coL.x z" "o.x z i.f#i.g" {
    quietly regress y `rhs'
    matrix expected_b = e(b)
    matrix expected_v = e(V)
    local expected_names : colnames e(b)
    generate byte expected_sample = e(sample)
    quietly fereg y `rhs', threads(1)
    local actual_names : colnames e(b)
    assert `"`actual_names'"' == `"`expected_names'"'
    assert e(sample) == expected_sample
    assert mreldif(e(b), expected_b) < 1e-9
    assert mreldif(e(V), expected_v) < 1e-9
    assert e(b)[1,1] == 0
    assert e(V)[1,1] == 0
    drop expected_sample

    quietly poisson yp `rhs', nolog
    matrix expected_b = e(b)
    local expected_names : colnames e(b)
    generate byte expected_sample = e(sample)
    quietly feppml yp `rhs', threads(1)
    local actual_names : colnames e(b)
    assert `"`actual_names'"' == `"`expected_names'"'
    assert e(sample) == expected_sample
    assert mreldif(e(b), expected_b) < 1e-7
    assert e(b)[1,1] == 0
    assert e(V)[1,1] == 0
    drop expected_sample
}

display as result "END_FIXTURE|stability061_explicit_omission|rc=0"
