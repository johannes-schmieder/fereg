version 18.0
clear all
set more off
set obs 400
generate long row = _n
generate double x = sin(_n)
generate double y = 2 + 3*x + cos(_n)
generate double yp = exp(.1*x) + mod(_n, 2)
generate double w = 1

* A random if expression is a direct witness that candidate evaluation occurs
* once. A constant expression weight and its stored equivalent must retain
* exactly the same original rows as the built-in commands at the same seed.
foreach restriction in "if runiform()<.5" "if runiform()<.5 in 11/350" {
    set seed 1965
    quietly regress y x [aw=1] `restriction'
    matrix expected_b = e(b)
    matrix expected_v = e(V)
    generate byte expected_sample = e(sample)
    local expected_n = e(N)
    foreach expression in "1" "w" "w+0" {
        set seed 1965
        quietly fereg y x [aw=`expression'] `restriction', threads(1)
        assert e(sample) == expected_sample
        assert e(N) == `expected_n'
        assert mreldif(e(b), expected_b) < 1e-10
        assert mreldif(e(V), expected_v) < 1e-10
        assert `"`e(wexp)'"' == "= `expression'"
        assert `"`e(cmdline)'"' == ///
            "fereg y x [aw=`expression'] `restriction', threads(1)"
    }
    drop expected_sample

    set seed 1965
    quietly poisson yp x [pw=1] `restriction', nolog
    matrix expected_b = e(b)
    generate byte expected_sample = e(sample)
    local expected_n = e(N)
    foreach expression in "1" "w" "w+0" {
        set seed 1965
        quietly feppml yp x [pw=`expression'] `restriction', threads(1)
        assert e(sample) == expected_sample
        assert e(N) == `expected_n'
        assert mreldif(e(b), expected_b) < 1e-8
        assert `"`e(wexp)'"' == "= `expression'"
        assert `"`e(cmdline)'"' == ///
            "feppml yp x [pw=`expression'] `restriction', threads(1)"
    }
    drop expected_sample
}

display as result "END_FIXTURE|stability061_weight_candidate_once|rc=0"
