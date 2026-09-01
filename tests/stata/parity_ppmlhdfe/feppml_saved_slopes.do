version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
clear all
set more off

* This fixture is shared by the hidden pre-activation adapter and the eventual
* public command.  Saved FE channels are observation-level contributions, so
* the intercept and slope channels must add directly to d.
set obs 144
generate long obs_id = _n
generate byte g = 1 + mod(_n - 1, 6)
generate byte h = 1 + mod(floor((_n - 1) / 3), 8)
generate byte within_g = floor((_n - 1) / 6)
generate double z = 0.35 + mod(5 * _n, 23) / 17
generate double w = 0.45 + mod(7 * _n, 29) / 19
generate double x = mod(11 * _n, 31) / 15 - 1
generate double eta_true = 0.18 * x + 0.04 * (g - 3) + ///
    0.025 * (g - 3) * z - 0.018 * (h - 4) + 0.022 * (h - 4) * w
generate double y = exp(eta_true) * (0.88 + 0.03 * mod(_n, 7))

* Intercept-plus-slope term, partial publication.  The declaration has three
* channels (g intercept, g slope, h intercept), but only the first two are
* requested.  The slope name is derived mechanically and no _Slope2 exists.
`feppml_test_command' y c.x, absorb(__is_part=g##c.z h) ///
    separation(none) keepsingletons memory(fast) d(__is_part_d) eta(__is_part_eta)
assert e(feppml_receipt)[1,11] == 3
assert e(feppml_receipt)[1,18] == 2
confirm numeric variable __is_part
confirm numeric variable __is_part_Slope1
confirm new variable __is_part_Slope2
bysort g (obs_id): assert abs(__is_part - __is_part[1]) <= 2e-8 if e(sample)
generate double __is_part_ratio = __is_part_Slope1 / z if e(sample)
bysort g (obs_id): assert abs(__is_part_ratio - __is_part_ratio[1]) <= 2e-8 if e(sample)
tempname is_part_b is_part_V
matrix `is_part_b' = e(b)
matrix `is_part_V' = e(V)
generate byte __is_part_sample = e(sample)
clonevar __is_part_copy = __is_part
clonevar __is_part_slope_copy = __is_part_Slope1
clonevar __is_part_d_copy = __is_part_d
clonevar __is_part_eta_copy = __is_part_eta
drop __is_part __is_part_Slope1 __is_part_d __is_part_eta

* Publishing every channel may not alter the normalized channels already
* requested, any scientific result, or the aggregate reconstruction.
`feppml_test_command' y c.x, absorb(__is_all=g##c.z __is_h=h) ///
    separation(none) keepsingletons memory(fast) d(__is_all_d) eta(__is_all_eta)
assert e(feppml_receipt)[1,11] == 3
assert e(feppml_receipt)[1,18] == 3
confirm numeric variable __is_all
confirm numeric variable __is_all_Slope1
confirm numeric variable __is_h
confirm new variable __is_all_Slope2
assert e(sample) == __is_part_sample
assert mreldif(e(b), `is_part_b') <= 1e-10
assert mreldif(e(V), `is_part_V') <= 1e-9
assert abs(__is_all - __is_part_copy) <= 2e-8 if e(sample)
assert abs(__is_all_Slope1 - __is_part_slope_copy) <= 2e-8 if e(sample)
assert abs(__is_all_d - __is_part_d_copy) <= 2e-8 if e(sample)
assert abs(__is_all_eta - __is_part_eta_copy) <= 2e-8 if e(sample)
assert abs(__is_all_d - __is_all - __is_all_Slope1 - __is_h) <= 2e-8 if e(sample)
assert abs(__is_all_eta - __is_all_d - ///
    (_b[_cons] + _b[x] * x)) <= 2e-8 if e(sample)

* Slope-only terms have no base output variable.  A partial request on the
* second declared term must stay on that declaration instead of shifting to
* the first compact output slot.
drop __is_all __is_all_Slope1 __is_h __is_all_d __is_all_eta
`feppml_test_command' y c.x, absorb(g#c.z __so_part=h#c.w) ///
    separation(none) keepsingletons memory(fast) d(__so_part_d) eta(__so_part_eta)
assert e(feppml_receipt)[1,11] == 2
assert e(feppml_receipt)[1,18] == 1
confirm new variable __so_part
confirm numeric variable __so_part_Slope1
confirm new variable __so_part_Slope2
generate double __so_part_ratio = __so_part_Slope1 / w if e(sample)
bysort h (obs_id): assert abs(__so_part_ratio - __so_part_ratio[1]) <= 2e-8 if e(sample)
tempname so_part_b so_part_V
matrix `so_part_b' = e(b)
matrix `so_part_V' = e(V)
generate byte __so_part_sample = e(sample)
clonevar __so_part_slope_copy = __so_part_Slope1
clonevar __so_part_d_copy = __so_part_d
clonevar __so_part_eta_copy = __so_part_eta
drop __so_part_Slope1 __so_part_d __so_part_eta

`feppml_test_command' y c.x, absorb(__so_g=g#c.z __so_h=h#c.w) ///
    separation(none) keepsingletons memory(fast) d(__so_all_d) eta(__so_all_eta)
assert e(feppml_receipt)[1,11] == 2
assert e(feppml_receipt)[1,18] == 2
confirm new variable __so_g
confirm new variable __so_h
confirm numeric variable __so_g_Slope1
confirm numeric variable __so_h_Slope1
confirm new variable __so_g_Slope2
confirm new variable __so_h_Slope2
assert e(sample) == __so_part_sample
assert mreldif(e(b), `so_part_b') <= 1e-10
assert mreldif(e(V), `so_part_V') <= 1e-9
assert abs(__so_h_Slope1 - __so_part_slope_copy) <= 2e-8 if e(sample)
assert abs(__so_all_d - __so_part_d_copy) <= 2e-8 if e(sample)
assert abs(__so_all_eta - __so_part_eta_copy) <= 2e-8 if e(sample)
assert abs(__so_all_d - __so_g_Slope1 - __so_h_Slope1) <= 2e-8 if e(sample)
assert abs(__so_all_eta - __so_all_d - ///
    (_b[_cons] + _b[x] * x)) <= 2e-8 if e(sample)

* Namespace conflicts fail before publication and preserve both populated e()
* and the requested-output namespace.  Cover base, derived slope, slope-only,
* and already-existing variable conflicts.
tempname accepted_b accepted_V
matrix `accepted_b' = e(b)
matrix `accepted_V' = e(V)
generate byte __accepted_sample = e(sample)

capture noisily `feppml_test_command' y c.x, ///
    absorb(__dupe=g##c.z) d(__dupe) separation(none)
assert _rc == 110
confirm new variable __dupe
confirm new variable __dupe_Slope1

capture noisily `feppml_test_command' y c.x, ///
    absorb(__dupe2=g##c.z) xb(__dupe2_Slope1) separation(none)
assert _rc == 110
confirm new variable __dupe2
confirm new variable __dupe2_Slope1

capture noisily `feppml_test_command' y c.x, ///
    absorb(__dupe3=g#c.z) mu(__dupe3_Slope1) separation(none)
assert _rc == 110
confirm new variable __dupe3
confirm new variable __dupe3_Slope1

capture noisily `feppml_test_command' y c.x, ///
    absorb(__dupe4=g __dupe4=h) separation(none)
assert _rc == 110
confirm new variable __dupe4

capture noisily `feppml_test_command' y c.x, ///
    absorb(__chain=g##c.z __chain_Slope1=h) separation(none)
assert _rc == 110
confirm new variable __chain
confirm new variable __chain_Slope1

generate double __occupied = 17
capture noisily `feppml_test_command' y c.x, ///
    absorb(__occupied=g##c.z) separation(none)
assert _rc == 198
assert __occupied == 17

generate double __future_Slope1 = 23
capture noisily `feppml_test_command' y c.x, ///
    absorb(__future=g##c.z) separation(none)
assert _rc == 198
confirm new variable __future
assert __future_Slope1 == 23

assert e(sample) == __accepted_sample
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_saved_slopes|rc=0"
