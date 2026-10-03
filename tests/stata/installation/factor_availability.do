version 18.0
clear all
set more off
args before
macro drop FEREG_ENGINE
foreach command in fereg feppml {
    foreach bad in -1 1.5 {
        foreach scenario in eligible missing_y missing_x missing_fe missing_cluster missing_slope missing_weight excluded_if omitted base {
            clear
            quietly set obs 120
            generate double y = 1 + mod(_n, 7) + _n/100
            generate double x = mod(_n * 13, 31) / 10
            generate double g = mod(_n, 3)
            generate byte firm = mod(_n, 4)
            generate byte cluster = mod(_n, 11)
            generate double z = 1 + mod(_n, 13) / 10
            generate double w = 1
            quietly replace g = `bad' in 1
            if "`scenario'" == "missing_y" quietly replace y = .a in 1
            if "`scenario'" == "missing_x" quietly replace x = . in 1
            if "`scenario'" == "missing_fe" quietly replace firm = . in 1
            if "`scenario'" == "missing_cluster" quietly replace cluster = . in 1
            if "`scenario'" == "missing_slope" quietly replace z = . in 1
            if "`scenario'" == "missing_weight" quietly replace w = . in 1
            local restriction
            if "`scenario'" == "excluded_if" local restriction "if _n > 1"
            local factor i.g
            if "`scenario'" == "omitted" local factor 1o.g
            if "`scenario'" == "base" local factor ib0.g
            local options absorb(firm##c.z) vce(cluster cluster) threads(1) deterministic
            foreach engine in legacy next {
                capture noisily `command' y `factor' x `restriction' [fw=w], `options' engine(`engine')
                local rc = _rc
                local expect = cond(inlist("`scenario'", "eligible", "omitted", "base"), 452, 0)
                if ("`engine'" == "legacy" | "`before'" == "before") & "`scenario'" != "excluded_if" local expect 452
                assert `rc' == `expect'
                if !`rc' {
                    assert e(N) == 119
                    assert e(sample) == (_n > 1)
                    matrix result_b = e(b)
                    matrix result_v = e(V)
                    quietly `command' y `factor' x if _n > 1 [fw=w], `options' engine(`engine')
                    assert mreldif(e(b), result_b) == 0 & mreldif(e(V), result_v) == 0
                }
            }
        }
    }
}
* Time-series availability uses the declared index, including data outside if.
clear
quietly set obs 120
generate t = _n
tsset t
generate double y = 1 + mod(_n, 7) + _n/100
generate double x = mod(_n * 13, 31) / 10
generate double g = mod(_n, 3)
generate byte firm = mod(_n, 4)
generate double w = 1
generate double adjustment = 1
quietly replace g = -1 in 1
foreach command in fereg feppml {
    quietly `command' y i.g L.x, absorb(firm) engine(next)
    assert e(N) == 119
    assert e(sample) == (_n > 1)
    capture noisily `command' y i.g L.x, absorb(firm) engine(legacy)
    assert _rc == 452
}
* Current missing x must not exclude a row whose lag is available.
quietly replace x = . in 120
quietly fereg y i.g L.x if _n > 1, absorb(firm) engine(next)
assert e(sample) == (_n > 1)
quietly replace x = 1 in 120
quietly replace adjustment = .a in 1
foreach option in offset exposure {
    quietly feppml y i.g x, absorb(firm) `option'(adjustment) engine(next)
    assert e(N) == 119
    capture noisily feppml y i.g x, absorb(firm) `option'(adjustment) engine(legacy)
    assert _rc == 452
}
quietly replace w = 0 in 1
quietly fereg y i.g x [aw=w], absorb(firm) engine(next)
assert e(N) == 119
capture noisily fereg y i.g x [aw=w], absorb(firm) engine(legacy)
assert _rc == 452
* Excluding a factor does not turn a weight-domain failure into a dropped row.
quietly replace y = . in 1
quietly replace w = -1 in 1
capture noisily fereg y i.g x [aw=w], absorb(firm) engine(next)
assert _rc == 402

* Compare a successful corrected fit with an independent explicit dummy model.
quietly replace w = 1 in 1
quietly regress y i.g x i.firm
scalar reference_x = _b[x]
quietly fereg y i.g x, absorb(firm) engine(next)
assert abs(_b[x] - reference_x) < 1e-10
quietly poisson y i.g x i.firm, nolog
scalar reference_x = _b[x]
quietly feppml y i.g x, absorb(firm) engine(next)
assert abs(_b[x] - reference_x) < 1e-8

di as result "END_FIXTURE|factor_availability|rc=0"
