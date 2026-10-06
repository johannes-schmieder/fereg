version 18.0
clear all
set more off
set obs 120
generate long obs_id = _n
generate byte g1 = ceil(_n / 10)
generate byte g2 = mod(_n, 4)
generate double x = mod(7 * _n, 13) / 5
generate double y = 1 + mod(3 * _n, 7)
replace y = 0 if g1 == 1

* The negative indicator for g1==1 is an exact recession direction: it
* changes only zero-outcome rows. Disabling proactive separation cannot
* produce a finite optimum or authorize partial publication.
quietly regress y x
tempname before_b before_V
matrix `before_b' = e(b)
matrix `before_V' = e(V)
generate byte before_sample = e(sample)
foreach storage in auto fast low {
    foreach workers in 1 2 4 7 8 9 15 16 {
        capture noisily feppml y x, absorb(saved1=g1 saved2=g2) ///
            separation(none) memory(`storage') threads(`workers') ///
            tolerance(1e-12) residuals(failed_resid) mu(failed_mu)
        assert _rc == 430
        assert `"`e(cmd)'"' == "regress"
        assert e(sample) == before_sample
        assert mreldif(e(b), `before_b') == 0
        assert mreldif(e(V), `before_V') == 0
        foreach output in saved1 saved2 failed_resid failed_mu {
            confirm new variable `output'
        }
    }
}
display as result "END_FIXTURE|feppml_separated_none_transaction|rc=0"
