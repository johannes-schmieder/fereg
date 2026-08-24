version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

quietly regress y c.x1 if obs_id <= 20
tempname prior_b prior_V prior_beta
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
matrix `prior_beta' = e(beta)
generate byte __frg_prior_sample = e(sample)
local prior_cmd `"`e(cmd)'"'
local prior_N : display %21x e(N)
local prior_rss : display %21x e(rss)

* The interactive PTY controller sends an actual Control-C only after this
* hidden action has completed its sole plugin call and printed its armed line.
capture noisily _fereg_estimate_ols_test y c.x1, ///
    absorb(worker firm) cluster(cluster_a cluster_b) ///
    residual(__frg_break_resid) inject(breakpostplugin)
local break_rc = _rc
assert `break_rc' == 1
confirm new variable __frg_break_resid
assert `"`e(cmd)'"' == `"`prior_cmd'"'
local after_N : display %21x e(N)
local after_rss : display %21x e(rss)
assert `"`after_N'"' == `"`prior_N'"'
assert `"`after_rss'"' == `"`prior_rss'"'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_V') == 0
assert mreldif(e(beta), `prior_beta') == 0
assert e(sample) == __frg_prior_sample

* Split the literal so the PTY controller cannot mistake Stata's command echo
* for the emitted terminal sentinel.
di as result "END_" "FIXTURE|estimate_ols_eclass_break|rc=0"
