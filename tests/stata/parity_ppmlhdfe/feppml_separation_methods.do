version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
clear all
set more off

* Singleton pruning has its own ledger reason and is not counted as
* statistical separation.
set obs 9
generate long obs_id = _n
generate byte fe = cond(_n == 1, 1, cond(_n <= 5, 2, 3))
generate double x = mod(3 * _n, 7) - 3
generate double y = 0.5 + mod(5 * _n, 9)

`feppml_test_command' y c.x, absorb(fe) separation(none) dropreason(__singleton_drop)
assert e(N_full) == 9
assert e(N_physical) == 8
assert e(separated) == 0
assert !e(sample) if obs_id == 1
assert e(sample) if obs_id > 1
assert __singleton_drop == 8 if obs_id == 1
assert __singleton_drop == 0 if obs_id > 1

* The same constructive finite-regressor recession direction is run through
* each numerical detector alone.  The first certifying method owns the stable
* public reason, while post-drop rank is recomputed on the final sample.
clear
set obs 20
generate long obs_id = _n
generate byte separated_cell = _n > 14
generate double x = mod(7 * _n, 11) - 5
generate double y = 0.5 + mod(3 * _n, 8)
replace y = 0 if separated_cell

foreach specification in "simplex 10 3" "ir 11 4" "mu 12 5" {
    gettoken method remainder : specification
    gettoken reason method_code : remainder
    capture drop __method_drop
    `feppml_test_command' y c.x c.separated_cell, separation(`method') ///
        dropreason(__method_drop)
    assert e(converged) == 1
    assert e(N_full) == 20
    assert e(N_physical) == 14
    assert e(separated) == 6
    assert e(sample) == !separated_cell
    assert __method_drop == `reason' if separated_cell
    assert __method_drop == 0 if !separated_cell
    assert e(feppml_receipt)[1,16] == 6
    assert colsof(e(feppml_separation)) == 14
    assert rowsof(e(feppml_separation)) == e(feppml_receipt)[1,17]
    mata: assert(any(st_matrix("e(feppml_separation)")[.,1] :== `method_code'))
    mata: assert(any((st_matrix("e(feppml_separation)")[.,1] :== `method_code') :& (st_matrix("e(feppml_separation)")[.,6] :== 6)))
    local names : colfullnames e(b)
    assert `"`names'"' == "x o.separated_cell _cons"
    assert e(feppml_status)[2,2] == 6
    assert e(b)[1,2] == 0
    forvalues index = 1/3 {
        assert e(V)[2,`index'] == 0
        assert e(V)[`index',2] == 0
    }
}

* If exact FE and finite-regressor methods can certify the same rows, the
* registered FE-first order is authoritative.
clear
set obs 18
generate long obs_id = _n
generate byte fe = ceil(_n / 6)
generate byte separated_cell = fe == 3
generate double x = mod(5 * _n, 9) - 4
generate double y = 0.5 + mod(7 * _n, 10)
replace y = 0 if separated_cell

`feppml_test_command' y c.x c.separated_cell, absorb(fe) ///
    separation(fe simplex ir) dropreason(__ordered_drop)
assert e(separated) == 6
assert e(sample) == !separated_cell
assert __ordered_drop == 9 if separated_cell
assert __ordered_drop == 0 if !separated_cell
mata: assert(any(st_matrix("e(feppml_separation)")[.,1] :== 2))

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_separation_methods|rc=0"
