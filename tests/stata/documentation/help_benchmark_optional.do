version 15.0
clear all
set more off

set obs 4
generate long original_id = _n
quietly datasignature set, reset
global GTOOLS_BETA "help fixture beta sentinel"
global GTOOLS_GREGTABLE "help fixture table sentinel"

mata: __help_existing = 42
fereg_run benchmark using fereg.sthlp
mata: assert(__help_existing == 42)
mata: mata drop __help_existing
quietly datasignature confirm
assert `"${GTOOLS_BETA}"' == "help fixture beta sentinel"
assert `"${GTOOLS_GREGTABLE}"' == "help fixture table sentinel"
estimates restore fereg
assert e(N) == 250000
assert e(runtime) >= 0 & e(runtime) < .
assert abs(_b[x1] - .6) < .01
assert abs(_b[x2] + .3) < .01
scalar reference_x1 = _b[x1]
scalar reference_x2 = _b[x2]
foreach model in reghdfe greg areg {
    capture estimates restore `model'
    if _rc continue
    assert e(N) == 250000
    assert e(runtime) >= 0 & e(runtime) < .
    assert reldif(_b[x1], reference_x1) < 1e-6
    assert reldif(_b[x2], reference_x2) < 1e-6
    assert _se[x1] > 0 & _se[x1] < .
    assert _se[x2] > 0 & _se[x2] < .
    if "`model'" == "greg" assert "`e(depvar)'" == "y"
}
global GTOOLS_BETA
global GTOOLS_GREGTABLE
di as result "END_FIXTURE|help_benchmark_optional|rc=0"
