version 18.0
clear all
set more off
args solver samples seed
if `"`solver'"' == "" local solver "auto"
assert inlist(`"`solver'"', "auto", "cmgschur")
local sampling_options
if `"`samples'"' != "" {
    assert `"`solver'"' == "cmgschur"
    local sampling_options "cmgsamples(`samples')"
}
if `"`seed'"' != "" {
    assert real(`"`samples'"') > 0
    local sampling_options "`sampling_options' cmgseed(`seed')"
}
local sampled = (`"`samples'"' != "" & real(`"`samples'"') > 0)

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))
local inference_options "cluster(cluster_a cluster_b)"
if `sampled' {
    * The post-plugin checkpoint follows a real weighted degree-30 sampled fit.
    * The native checkpoint still waits before numerical preparation starts.
    set obs 180
    generate long worker = _n
    expand 60
    bysort worker: generate byte period = _n
    generate long obs_id = _n
    generate int firm = 1 + mod(worker * 13 + floor((period - 1)/2) * 7, 61)
    generate double x1 = sin(_n*.31)
    generate double weight = 1 + mod(_n,7)/3
    generate double y = .7*x1 + sin(worker*.11) + cos(firm*.3) + ///
        (mod(_n*37,101)-50)*.001
    local inference_options "vce(robust) aweight(weight)"
}
else import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

quietly regress y c.x1 if obs_id <= 20
tempname prior_b prior_V prior_beta
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
matrix `prior_beta' = e(beta)
generate byte __frg_prior_sample = e(sample)
local prior_cmd `"`e(cmd)'"'
local prior_N : display %21x e(N)
local prior_rss : display %21x e(rss)

* Native Break: the numerical phase waits at a cancellation checkpoint; the PTY
* controller sends Control-C after the native marker file appears. Stata's
* thread observes Break, cancels the work and the transaction rolls back.
capture noisily _fereg_estimate_ols_test y c.x1, ///
    absorb(worker firm) `inference_options' ///
    solver(`solver') `sampling_options' residual(__frg_native_resid) inject(breaknative)
local native_rc = _rc
assert `native_rc' == 1
confirm new variable __frg_native_resid
assert `"`e(cmd)'"' == `"`prior_cmd'"'
local after_N : display %21x e(N)
local after_rss : display %21x e(rss)
assert `"`after_N'"' == `"`prior_N'"'
assert `"`after_rss'"' == `"`prior_rss'"'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_V') == 0
assert e(sample) == __frg_prior_sample

* The interactive PTY controller sends an actual Control-C only after this
* hidden action has completed its sole plugin call and printed its armed line.
capture noisily _fereg_estimate_ols_test y c.x1, ///
    absorb(worker firm) `inference_options' ///
    solver(`solver') `sampling_options' residual(__frg_break_resid) inject(breakpostplugin)
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
if `sampled' di as result "FEREG_SAMPLED_" "BREAK_VERIFIED"

* Split the literal so the PTY controller cannot mistake Stata's command echo
* for the emitted terminal sentinel.
di as result "END_" "FIXTURE|estimate_ols_eclass_break|rc=0"
