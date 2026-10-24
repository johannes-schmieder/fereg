version 18.0
clear
set more off

* The runner installs a private forwarder around the real transaction. Both
* public dispatchers remain unchanged; only break_marker_token enters the wire.
* This is the native pre-preparation checkpoint, not a cell-builder interruption.
quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear
local host_processors = min(4, c(processors_lic), c(processors_mach))

quietly regress y c.x1 if obs_id <= 20
tempname prior_b prior_V
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
generate byte __frg_prior_sample = e(sample)
local prior_cmd `"`e(cmd)'"'
local prior_N : display %21x e(N)
local prior_rss : display %21x e(rss)
quietly datasignature set, reset

global FEREG_PUBLIC_BREAK_ARMED 1
capture noisily fereg y c.x1, absorb(__frg_public_fe=worker firm) ///
    solver(auto) preconditioner(auto) memory(fast) vce(robust) ///
    threads(`host_processors') residuals(__frg_public_resid)
local break_rc = _rc
global FEREG_PUBLIC_BREAK_ARMED 0
assert `break_rc' == 1
confirm new variable __frg_public_resid
confirm new variable __frg_public_fe
quietly datasignature confirm
assert `"`e(cmd)'"' == `"`prior_cmd'"'
local after_N : display %21x e(N)
local after_rss : display %21x e(rss)
assert `"`after_N'"' == `"`prior_N'"'
assert `"`after_rss'"' == `"`prior_rss'"'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_V') == 0
assert e(sample) == __frg_prior_sample
capture confirm matrix e(fereg_receipt)
assert _rc == 111

* A subsequent ordinary public fit must not inherit cancellation or the marker.
quietly fereg y c.x1, absorb(worker firm) solver(auto) preconditioner(auto) ///
    memory(fast) vce(robust) threads(`host_processors') residuals(__frg_after_resid)
assert `"`e(cmd)'"' == "fereg"
assert `"`e(fereg_gate)'"' == "public_ols_v1"
confirm variable __frg_after_resid
macro drop FEREG_PUBLIC_BREAK_ARMED
di as result "FEREG_PUBLIC_" "SCOPE_VERIFIED"
di as result "END_" "FIXTURE|estimate_ols_public_break|rc=0"
