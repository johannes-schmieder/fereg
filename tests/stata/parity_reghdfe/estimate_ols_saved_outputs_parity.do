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
* 20260824T161810Z-9f2494d, case saved_outputs.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(__frg_fe_worker=worker __frg_fe_firm=firm) ///
    residuals(__frg_resid) xb(__frg_xb) xbd(__frg_xbd) d(__frg_d)

assert e(N) == 24
assert e(df_a) == 18
assert e(df_r) == 5
assert e(sample) == 1
assert !missing(__frg_resid, __frg_xb, __frg_xbd, __frg_d, ///
    __frg_fe_worker, __frg_fe_firm)
assert abs(y - __frg_xb - __frg_d - __frg_resid) <= 1e-10
assert abs(__frg_xbd - __frg_xb - __frg_d) <= 1e-10
assert abs(__frg_d - __frg_fe_worker - __frg_fe_firm) <= 1e-10

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "x1 _cons"
assert abs(e(b)[1,1] - +1.9ef69c828f53dX+000) <= 1e-10
assert abs(e(b)[1,2] - +1.8068af367ae1dX+002) <= 1e-10
assert abs(e(V)[1,1] - +1.933e5cda254a2X-006) <= 1e-10
assert abs(e(V)[1,2] - -1.d6736c53d62bcX-008) <= 1e-10
assert abs(e(V)[2,1] - -1.d6736c53d62bcX-008) <= 1e-10
assert abs(e(V)[2,2] - +1.50a9e18a3a32eX-009) <= 1e-10
assert abs(e(rss) - +1.d2c1219deb7d6X-005) <= 1e-10

* Independently observable normalization and categorical-channel identities.
quietly summarize __frg_fe_worker, meanonly
assert abs(r(mean)) <= 1e-10
quietly summarize __frg_fe_firm, meanonly
assert abs(r(mean)) <= 1e-10
bysort worker (__frg_fe_worker): assert ///
    abs(__frg_fe_worker - __frg_fe_worker[1]) <= 1e-12
bysort firm (__frg_fe_firm): assert ///
    abs(__frg_fe_firm - __frg_fe_firm[1]) <= 1e-12

* Registered exact-reference anchors for the first, second, and final rows.
assert abs(__frg_resid - -1.f3ca830fbd2b0X-005) <= 1e-10 if obs_id == 1
assert abs(__frg_xb - +1.c998690b0a44cX+001) <= 1e-10 if obs_id == 1
assert abs(__frg_xbd - +1.10b461061f7a5X+002) <= 1e-10 if obs_id == 1
assert abs(__frg_d - +1.5f416404d2bfbX-001) <= 1e-10 if obs_id == 1
assert abs(__frg_fe_worker - +1.6b198d835ef07X-001) <= 1e-7 if obs_id == 1
assert abs(__frg_fe_firm - -1.7b052fd186043X-006) <= 1e-7 if obs_id == 1

assert abs(__frg_resid - +1.f3ca830fbd310X-005) <= 1e-10 if obs_id == 2
assert abs(__frg_xb - +1.0e4b4402ae018X+002) <= 1e-10 if obs_id == 2
assert abs(__frg_xbd - +1.38e536f9e085aX+002) <= 1e-10 if obs_id == 2
assert abs(__frg_d - +1.54cf97b99420fX-001) <= 1e-10 if obs_id == 2
assert abs(__frg_fe_worker - +1.6b198d835ef07X-001) <= 1e-7 if obs_id == 2
assert abs(__frg_fe_firm - -1.649f5c9cad013X-005) <= 1e-7 if obs_id == 2

assert abs(__frg_resid - +1.bb4eabbaa2bebX-006) <= 1e-10 if obs_id == 24
assert abs(__frg_xb - +1.27f1febbe145eX+003) <= 1e-10 if obs_id == 24
assert abs(__frg_xbd - +1.172258aa22aeaX+003) <= 1e-10 if obs_id == 24
assert abs(__frg_d - -1.0cfa611be973fX-001) <= 1e-10 if obs_id == 24
assert abs(__frg_fe_worker - -1.17f81671e98ffX-001) <= 1e-7 if obs_id == 24
assert abs(__frg_fe_firm - +1.5fb6aac003969X-006) <= 1e-7 if obs_id == 24

predict double __frg_predict_xb, xb
predict double __frg_predict_xbd, xbd
predict double __frg_predict_d, d
predict double __frg_predict_resid, residuals
assert abs(__frg_predict_xb - __frg_xb) <= 1e-12
assert abs(__frg_predict_xbd - __frg_xbd) <= 1e-12
assert abs(__frg_predict_d - __frg_d) <= 1e-10
assert abs(__frg_predict_resid - __frg_resid) <= 1e-12
capture noisily predict double __frg_invalid, xb d
assert _rc == 198
confirm new variable __frg_invalid

drop __frg_resid __frg_xb __frg_xbd __frg_d ///
    __frg_fe_worker __frg_fe_firm __frg_predict_*
fereg y c.x1, absorb(__frg_fe_worker=worker __frg_fe_firm=firm) ///
    residuals(__frg_resid) memory(fast)
assert `"`e(cmd)'"' == "fereg"
assert `"`e(fereg_memory_mode)'"' == "fast"
predict double __frg_xb, xb
predict double __frg_xbd, xbd
predict double __frg_d, d
predict double __frg_predict_resid, residuals
assert abs(y - __frg_xb - __frg_d - __frg_resid) <= 1e-10
assert abs(__frg_xbd - __frg_xb - __frg_d) <= 1e-10
assert abs(__frg_d - __frg_fe_worker - __frg_fe_firm) <= 1e-10
assert abs(__frg_predict_resid - __frg_resid) <= 1e-12

* Every explicit solver must publish the same certified normalized FE channels.
clonevar __frg_base_worker = __frg_fe_worker
clonevar __frg_base_firm = __frg_fe_firm
scalar __frg_base_b = _b[x1]
scalar __frg_base_rss = e(rss)
foreach solver in map cg it squarem lsmr cmg {
    capture drop __frg_route_*
    fereg y c.x1, ///
        absorb(__frg_route_worker=worker __frg_route_firm=firm) ///
        residuals(__frg_route_resid) solver(`solver') memory(fast)
    assert reldif(_b[x1], __frg_base_b) <= 1e-10
    assert reldif(e(rss), __frg_base_rss) <= 1e-10
    predict double __frg_route_xb, xb
    predict double __frg_route_d, d
    assert abs(y - __frg_route_xb - __frg_route_d - __frg_route_resid) <= 1e-10
    assert abs(__frg_route_d - __frg_route_worker - __frg_route_firm) <= 1e-10
    assert abs(__frg_route_worker - __frg_base_worker) <= 1e-7
    assert abs(__frg_route_firm - __frg_base_firm) <= 1e-7
}
drop __frg_route_* __frg_base_worker __frg_base_firm

di as result "END_FIXTURE|estimate_ols_saved_outputs_parity|rc=0"
