version 18.0
clear all
set obs 40
gen long id = ceil(_n/10)
gen int t = mod(_n-1,10)+1
gen double x = _n
tsset id t
tempvar touse
gen byte `touse' = 1

capture noisily stfixest_parse L.x F2.x, touse(`touse')
if _rc == 0 {
    matrix M = r(model_terms)
    assert r(n_model_terms) >= 2
    matrix list M
}
else {
    di as error "Record this parser failure before Phase 2; do not silently materialize all FV terms."
    exit _rc
}
di as result "time_series_marker.do: PASS"
di as result "END_FIXTURE|time_series_marker|rc=0"
