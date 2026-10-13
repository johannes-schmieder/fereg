version 18.0
clear all
set more off
set varabbrev off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* One installation diagnostic retains the frozen wire identities.
display as text "FEREG_F0_OUTPUT_BEGIN|selftest"
capture noisily fereg, selftest
assert _rc == 0
assert `"`r(ado_version)'"' == "0.5.0"
assert `"`r(action_protocol)'"' == "1.6"
assert `"`r(model_wire)'"' == "2.1"
assert r(requested_threads) == 0
capture noisily fereg, selftest threads(1) verbose
assert _rc == 0
assert r(requested_threads) == 1
display as text "FEREG_F0_OUTPUT_END|selftest"
foreach retired in version doctor {
    capture noisily fereg, `retired'
    assert _rc == 198
}
capture noisily fereg, selftest threads(-1)
assert _rc == 198

import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

* Validation failures preserve an unrelated estimation result exactly.
quietly regress y c.x1 if obs_id <= 20
generate byte __f0_prior_sample = e(sample)
tempname prior_b prior_v
matrix `prior_b' = e(b)
matrix `prior_v' = e(V)
local prior_cmd `"`e(cmd)'"'
local prior_N = e(N)
local prior_df_r = e(df_r)

capture noisily fereg y c.x1, absorb(worker firm) threads(-1)
assert _rc == 198
capture noisily fereg y c.x1, absorb(worker firm) memory(unknown)
assert _rc == 198
capture noisily fereg y c.x1, absorb(worker firm) ///
    solver(map) preconditioner(diagonal)
assert _rc == 198
capture noisily fereg y c.x1 [iw=x1], absorb(worker firm)
assert _rc == 101
assert `"`e(cmd)'"' == `"`prior_cmd'"'
assert e(N) == `prior_N'
assert e(df_r) == `prior_df_r'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_v') == 0
assert e(sample) == __f0_prior_sample

* A post-hold scientific failure must also restore e() and publish no output.
capture noisily fereg y c.x1, absorb(firm##c.time worker) ///
    residuals(__f0_forbidden_residual)
assert _rc == 2001
confirm new variable __f0_forbidden_residual
assert `"`e(cmd)'"' == `"`prior_cmd'"'
assert e(N) == `prior_N'
assert e(df_r) == `prior_df_r'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_v') == 0
assert e(sample) == __f0_prior_sample
capture confirm matrix e(fereg_receipt)
assert _rc == 111

* Defaults, public metadata, test, and estat remain unchanged.
quietly fereg y c.x1 c.x2, absorb(worker firm) deterministic
assert `"`e(cmd)'"' == "fereg"
assert `"`e(vce)'"' == "ols"
assert `"`e(fereg_memory_mode)'"' == "auto"
assert `"`e(fereg_solver_requested)'"' == "auto"
assert `"`e(fereg_preconditioner)'"' == "auto"
assert `"`e(fereg_action_protocol)'"' == "1.6"
assert `"`e(fereg_model_wire)'"' == "2.1"
assert `"`e(fereg_result_wire)'"' == "1.5"
assert `"`e(predict)'"' == "fereg_p"
assert e(fereg_requested_threads) == 0
assert e(fereg_receipt)[1,16] == 1

capture noisily test x1
assert _rc == 0
assert r(df) == 1
capture noisily estat summarize y x1 x2
assert _rc == 0
assert rowsof(r(stats)) == 3
assert colsof(r(stats)) == 4
capture noisily estat nonsense
assert _rc == 321

* Requested outputs and every public predict mode retain their identities.
drop __f0_prior_sample
quietly fereg y c.x1, ///
    absorb(__f0_worker=worker __f0_firm=firm) ///
    residuals(__f0_residual) memory(fast) threads(1) deterministic
predict double __f0_xb, xb
predict double __f0_xbd, xbd
predict double __f0_d, d
predict double __f0_predict_residual, residuals
assert abs(y - __f0_xb - __f0_d - __f0_residual) <= 1e-10
assert abs(__f0_xbd - __f0_xb - __f0_d) <= 1e-10
assert abs(__f0_d - __f0_worker - __f0_firm) <= 1e-10
assert abs(__f0_predict_residual - __f0_residual) <= 1e-12
capture noisily predict double __f0_invalid_prediction, xb d
assert _rc == 198
confirm new variable __f0_invalid_prediction

di as result "fereg_f0_public_contract.do: PASS"
di as result "END_FIXTURE|fereg_f0_public_contract|rc=0"
