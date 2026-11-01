version 17.0
clear all
set more off

* Live Break during parallel numeric loading. The hidden input_break injection
* completes the first parallel phase (every worker joined, window closed),
* publishes the native marker and polls on Stata's thread; the PTY controller
* then sends a real Control-C. The transaction must roll back, and the same
* session must load and estimate again exactly.
local workers = min(4, c(processors_mach), c(processors_lic))
assert `workers' >= 2
set obs 3000000
generate long row = _n
generate double x1 = sin(_n * .37)
generate float x2 = mod(_n, 97) / 7
generate long x3 = mod(_n * 13, 100003)
generate int x4 = mod(_n * 7, 30011) - 15000
generate byte x5 = mod(_n, 120) - 60
generate double y = x1 + .1 * x2 + 1e-6 * x3 + 1e-5 * x4 + .01 * x5 + sin(_n * .011)
generate double py = exp(.2 * x1 + .01 * x2) * (1 + mod(_n, 3))
generate long g = mod(_n * 31, 5003)
generate byte keep = mod(_n, 5) != 2

quietly regress y x1 if row <= 20
tempname prior_b prior_V
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
local prior_cmd `"`e(cmd)'"'
generate byte __prior_sample = e(sample)

capture noisily _fereg_estimate_ols_test y x1 x2 x3 x4 x5 if keep, ///
    absorb(g) threads(`workers') residual(__failed_resid) inject(input_break)
assert _rc == 1
confirm new variable __failed_resid
assert `"`e(cmd)'"' == `"`prior_cmd'"'
assert mreldif(e(b), `prior_b') == 0
assert mreldif(e(V), `prior_V') == 0
assert e(sample) == __prior_sample

capture noisily _fereg_estimate_ppml py x1 x2 x5 if keep, absorb(g) ///
    threads(`workers') mu(__failed_mu) inject(input_break)
assert _rc == 1
confirm new variable __failed_mu
assert `"`e(cmd)'"' == `"`prior_cmd'"'
assert mreldif(e(b), `prior_b') == 0

* The same session reads in parallel again; equal worker counts are exact.
quietly _fereg_estimate_ols_test y x1 x2 x3 x4 x5 if keep, absorb(g) ///
    threads(`workers') inject(serial_input)
tempname serial_b serial_V
matrix `serial_b' = e(b)
matrix `serial_V' = e(V)
quietly _fereg_estimate_ols_test y x1 x2 x3 x4 x5 if keep, absorb(g) ///
    threads(`workers') residual(__followup_resid)
assert mreldif(e(b), `serial_b') == 0
assert mreldif(e(V), `serial_V') == 0
confirm variable __followup_resid
di as result "FEREG_INPUT_BREAK_" "RESET_VERIFIED"

di as result "END_" "FIXTURE|estimate_input_break|rc=0"
