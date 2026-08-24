version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* Frozen licensed comparator:
* benchmarks/reports/captured/darwin-arm64/stata18/ols/
* 20260824T161810Z-9f2494d, case postestimation_test_summarize.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

fereg y c.x1 c.x2, absorb(worker firm) memory(auto) threads(2)
assert `"`e(cmd)'"' == "fereg"
assert `"`e(fereg_memory_mode)'"' == "auto"
assert e(fereg_requested_threads) == 2
assert e(N) == 24
assert e(df_m) == 2
assert e(df_r) == 4

test x1
assert abs(r(F) - +1.94ef8591c969dX+004) <= 1e-7
assert r(df) == 1
assert r(df_r) == 4
assert abs(r(p) - +1.e05a5494624e5X-008) <= 1e-10

estat summarize
matrix __frg_expected_stats = ///
    (+1.9eaaaaa000000X+002, +1.7fa094ba3df16X+000, +1.e666660000000X+001, +1.1b33340000000X+003 \ ///
     +1.2aaaaaaaaaaabX-002, +1.12ad29836e593X+000, -1.8000000000000X+000, +1.0000000000000X+001 \ ///
     -1.444446aaaaaabX-004, +1.49ae0ed72dd2fX+000, -1.0000000000000X+001, +1.d9999a0000000X+000)
matrix rownames __frg_expected_stats = y x1 x2
matrix colnames __frg_expected_stats = mean sd min max
assert rowsof(r(stats)) == 3
assert colsof(r(stats)) == 4
assert mreldif(r(stats), __frg_expected_stats) <= 1e-12

capture noisily estat nonsense
assert _rc == 321

di as result "END_FIXTURE|estimate_ols_postestimation_parity|rc=0"
