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


* Phase-7 accepted-method matrix. User order is syntax; execution and
* authoritative reason precedence always use the registered order.
capture program drop __phase7_sequence_case
program define __phase7_sequence_case
    version 18.0
    args case_id methods expected_reason fe_requested simplex_requested ir_requested mu_requested
    local phase7_command `"$FEREG_PPML_TEST_COMMAND"'
    if strtrim(`"`phase7_command'"') == "" local phase7_command "feppml"
    capture drop __p7_sequence_drop
    quietly `phase7_command' y c.x c.separated_cell, absorb(fe) ///
        separation(`methods') dropreason(__p7_sequence_drop) ///
        threads(1) deterministic
    assert e(converged) == 1
    assert e(N_full) == 18
    assert e(N_physical) == 12
    assert e(separated) == 6
    assert e(sample) == !separated_cell
    assert __p7_sequence_drop == `expected_reason' if separated_cell
    assert __p7_sequence_drop == 0 if !separated_cell
    assert e(feppml_separation_sequence)[2,2] == `fe_requested'
    assert e(feppml_separation_sequence)[3,2] == `simplex_requested'
    assert e(feppml_separation_sequence)[4,2] == `ir_requested'
    assert e(feppml_separation_sequence)[5,2] == `mu_requested'
    assert e(feppml_separation_sequence)[2,3] == cond(`fe_requested', 1, 0)
    assert e(feppml_separation_sequence)[3,3] == cond(`simplex_requested', 2, 0)
    assert e(feppml_separation_sequence)[4,3] == cond(`ir_requested', 3, 0)
    assert e(feppml_separation_sequence)[5,3] == cond(`mu_requested', 4, 0)
    display as result "PHASE7_SEQUENCE|`case_id'|`methods'|rc=0"
end

__phase7_sequence_case p7_sequence_001 "fe" 9 1 0 0 0
__phase7_sequence_case p7_sequence_002 "simplex" 10 0 1 0 0
__phase7_sequence_case p7_sequence_003 "ir" 11 0 0 1 0
__phase7_sequence_case p7_sequence_004 "mu" 12 0 0 0 1
__phase7_sequence_case p7_sequence_005 "fe simplex" 9 1 1 0 0
__phase7_sequence_case p7_sequence_006 "fe ir" 9 1 0 1 0
__phase7_sequence_case p7_sequence_007 "fe mu" 9 1 0 0 1
__phase7_sequence_case p7_sequence_008 "simplex fe" 9 1 1 0 0
__phase7_sequence_case p7_sequence_009 "simplex ir" 10 0 1 1 0
__phase7_sequence_case p7_sequence_010 "simplex mu" 10 0 1 0 1
__phase7_sequence_case p7_sequence_011 "ir fe" 9 1 0 1 0
__phase7_sequence_case p7_sequence_012 "ir simplex" 10 0 1 1 0
__phase7_sequence_case p7_sequence_013 "ir mu" 11 0 0 1 1
__phase7_sequence_case p7_sequence_014 "mu fe" 9 1 0 0 1
__phase7_sequence_case p7_sequence_015 "mu simplex" 10 0 1 0 1
__phase7_sequence_case p7_sequence_016 "mu ir" 11 0 0 1 1
__phase7_sequence_case p7_sequence_017 "fe simplex ir" 9 1 1 1 0
__phase7_sequence_case p7_sequence_018 "fe simplex mu" 9 1 1 0 1
__phase7_sequence_case p7_sequence_019 "fe ir simplex" 9 1 1 1 0
__phase7_sequence_case p7_sequence_020 "fe ir mu" 9 1 0 1 1
__phase7_sequence_case p7_sequence_021 "fe mu simplex" 9 1 1 0 1
__phase7_sequence_case p7_sequence_022 "fe mu ir" 9 1 0 1 1
__phase7_sequence_case p7_sequence_023 "simplex fe ir" 9 1 1 1 0
__phase7_sequence_case p7_sequence_024 "simplex fe mu" 9 1 1 0 1
__phase7_sequence_case p7_sequence_025 "simplex ir fe" 9 1 1 1 0
__phase7_sequence_case p7_sequence_026 "simplex ir mu" 10 0 1 1 1
__phase7_sequence_case p7_sequence_027 "simplex mu fe" 9 1 1 0 1
__phase7_sequence_case p7_sequence_028 "simplex mu ir" 10 0 1 1 1
__phase7_sequence_case p7_sequence_029 "ir fe simplex" 9 1 1 1 0
__phase7_sequence_case p7_sequence_030 "ir fe mu" 9 1 0 1 1
__phase7_sequence_case p7_sequence_031 "ir simplex fe" 9 1 1 1 0
__phase7_sequence_case p7_sequence_032 "ir simplex mu" 10 0 1 1 1
__phase7_sequence_case p7_sequence_033 "ir mu fe" 9 1 0 1 1
__phase7_sequence_case p7_sequence_034 "ir mu simplex" 10 0 1 1 1
__phase7_sequence_case p7_sequence_035 "mu fe simplex" 9 1 1 0 1
__phase7_sequence_case p7_sequence_036 "mu fe ir" 9 1 0 1 1
__phase7_sequence_case p7_sequence_037 "mu simplex fe" 9 1 1 0 1
__phase7_sequence_case p7_sequence_038 "mu simplex ir" 10 0 1 1 1
__phase7_sequence_case p7_sequence_039 "mu ir fe" 9 1 0 1 1
__phase7_sequence_case p7_sequence_040 "mu ir simplex" 10 0 1 1 1
__phase7_sequence_case p7_sequence_041 "fe simplex ir mu" 9 1 1 1 1
__phase7_sequence_case p7_sequence_042 "fe simplex mu ir" 9 1 1 1 1
__phase7_sequence_case p7_sequence_043 "fe ir simplex mu" 9 1 1 1 1
__phase7_sequence_case p7_sequence_044 "fe ir mu simplex" 9 1 1 1 1
__phase7_sequence_case p7_sequence_045 "fe mu simplex ir" 9 1 1 1 1
__phase7_sequence_case p7_sequence_046 "fe mu ir simplex" 9 1 1 1 1
__phase7_sequence_case p7_sequence_047 "simplex fe ir mu" 9 1 1 1 1
__phase7_sequence_case p7_sequence_048 "simplex fe mu ir" 9 1 1 1 1
__phase7_sequence_case p7_sequence_049 "simplex ir fe mu" 9 1 1 1 1
__phase7_sequence_case p7_sequence_050 "simplex ir mu fe" 9 1 1 1 1
__phase7_sequence_case p7_sequence_051 "simplex mu fe ir" 9 1 1 1 1
__phase7_sequence_case p7_sequence_052 "simplex mu ir fe" 9 1 1 1 1
__phase7_sequence_case p7_sequence_053 "ir fe simplex mu" 9 1 1 1 1
__phase7_sequence_case p7_sequence_054 "ir fe mu simplex" 9 1 1 1 1
__phase7_sequence_case p7_sequence_055 "ir simplex fe mu" 9 1 1 1 1
__phase7_sequence_case p7_sequence_056 "ir simplex mu fe" 9 1 1 1 1
__phase7_sequence_case p7_sequence_057 "ir mu fe simplex" 9 1 1 1 1
__phase7_sequence_case p7_sequence_058 "ir mu simplex fe" 9 1 1 1 1
__phase7_sequence_case p7_sequence_059 "mu fe simplex ir" 9 1 1 1 1
__phase7_sequence_case p7_sequence_060 "mu fe ir simplex" 9 1 1 1 1
__phase7_sequence_case p7_sequence_061 "mu simplex fe ir" 9 1 1 1 1
__phase7_sequence_case p7_sequence_062 "mu simplex ir fe" 9 1 1 1 1
__phase7_sequence_case p7_sequence_063 "mu ir fe simplex" 9 1 1 1 1
__phase7_sequence_case p7_sequence_064 "mu ir simplex fe" 9 1 1 1 1

__phase7_sequence_case p7_default "default" 9 1 1 1 0
clear
set obs 8
generate long obs_id = _n
generate byte fe = ceil(_n / 4)
generate double x = mod(5 * _n, 7) - 3
generate double y = 1 + mod(3 * _n, 5)
`feppml_test_command' y c.x, absorb(fe) separation(none)
assert e(separated) == 0
assert e(N_physical) == 8

* Manifest-generated exact recession-cone recipes. The Python source contract
* requires each complete block below to match its closed manifest row-for-row.
* PHASE7_RECIPE_BEGIN|finite_indicator
clear
input long obs_id double(y x separated_cell)
1 1 2 0
2 1 -2 0
3 1 5 0
4 1 1 0
5 1 -3 0
6 1 4 0
7 1 0 0
8 1 -4 0
9 1 3 0
10 1 -1 0
11 1 -5 0
12 1 2 0
13 1 -2 0
14 1 5 0
15 0 1 1
16 0 -3 1
17 0 4 1
18 0 0 1
19 0 -4 1
20 0 3 1
end
assert _N == 20
`feppml_test_command' y c.x c.separated_cell, separation(simplex) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 6
assert e(N_physical) == 14
generate byte __p7_expected_drop = inlist(obs_id, 15, 16, 17, 18, 19, 20)
assert e(sample) == !__p7_expected_drop
assert __p7_case_drop == 10 if __p7_expected_drop
assert __p7_case_drop == 0 if !__p7_expected_drop
display as result "PHASE7_ORACLE|finite_indicator|numerical_cone"
* PHASE7_RECIPE_END|finite_indicator

* PHASE7_RECIPE_BEGIN|full_positive_rank
clear
input long obs_id double(y)
1 0
2 1
3 2
end
assert _N == 3
`feppml_test_command' y, separation(simplex) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 0
assert e(N_physical) == 3
assert e(sample)
assert __p7_case_drop == 0
display as result "PHASE7_ORACLE|full_positive_rank|full_positive_rank"
* PHASE7_RECIPE_END|full_positive_rank

* PHASE7_RECIPE_BEGIN|boundary_infeasible
clear
input long obs_id double(y x)
1 1 0
2 2 0
3 0 1
4 0 -1
end
assert _N == 4
`feppml_test_command' y c.x, separation(simplex) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 0
assert e(N_physical) == 4
assert e(sample)
assert __p7_case_drop == 0
assert e(separated) == 0
display as result "PHASE7_ORACLE|boundary_infeasible|cone_infeasible"
* PHASE7_RECIPE_END|boundary_infeasible

* PHASE7_RECIPE_BEGIN|duplicate_degenerate
clear
input long obs_id double(y x xdup)
1 1 0 0
2 2 0 0
3 0 1 2
4 0 1 2
5 3 0 0
end
assert _N == 5
`feppml_test_command' y c.x c.xdup, separation(simplex) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 2
assert e(N_physical) == 3
generate byte __p7_expected_drop = inlist(obs_id, 3, 4)
assert e(sample) == !__p7_expected_drop
assert __p7_case_drop == 10 if __p7_expected_drop
assert __p7_case_drop == 0 if !__p7_expected_drop
display as result "PHASE7_ORACLE|duplicate_degenerate|simplex_degenerate"
* PHASE7_RECIPE_END|duplicate_degenerate

* PHASE7_RECIPE_BEGIN|fe_only
clear
input long obs_id double(y fe)
1 1 1
2 2 1
3 0 2
4 0 2
end
assert _N == 4
`feppml_test_command' y, absorb(fe) separation(simplex) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 2
assert e(N_physical) == 2
generate byte __p7_expected_drop = inlist(obs_id, 3, 4)
assert e(sample) == !__p7_expected_drop
assert __p7_case_drop == 10 if __p7_expected_drop
assert __p7_case_drop == 0 if !__p7_expected_drop
display as result "PHASE7_ORACLE|fe_only|simplex_fe_only"
* PHASE7_RECIPE_END|fe_only

* PHASE7_RECIPE_BEGIN|mixed_finite_fe
clear
input long obs_id double(y sep fe)
1 1 0 1
2 0 0 1
3 0 1 1
4 2 0 2
5 0 0 2
6 0 1 2
end
assert _N == 6
`feppml_test_command' y c.sep, absorb(fe) separation(ir) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 2
assert e(N_physical) == 4
generate byte __p7_expected_drop = inlist(obs_id, 3, 6)
assert e(sample) == !__p7_expected_drop
assert __p7_case_drop == 11 if __p7_expected_drop
assert __p7_case_drop == 0 if !__p7_expected_drop
display as result "PHASE7_ORACLE|mixed_finite_fe|simplex_mixed"
* PHASE7_RECIPE_END|mixed_finite_fe

* PHASE7_RECIPE_BEGIN|absorbed_slope
clear
input long obs_id double(y sep fe slope)
1 1 0 1 0
2 2 0 1 1
3 3 0 1 2
4 0 1 1 0
5 0 1 1 1
6 1 0 2 0
7 2 0 2 1
8 3 0 2 2
9 0 1 2 0
10 0 1 2 1
end
assert _N == 10
`feppml_test_command' y c.sep, absorb(fe##c.slope) separation(simplex) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 4
assert e(N_physical) == 6
generate byte __p7_expected_drop = inlist(obs_id, 4, 5, 9, 10)
assert e(sample) == !__p7_expected_drop
assert __p7_case_drop == 10 if __p7_expected_drop
assert __p7_case_drop == 0 if !__p7_expected_drop
display as result "PHASE7_ORACLE|absorbed_slope|simplex_absorbed_slope"
* PHASE7_RECIPE_END|absorbed_slope

* PHASE7_RECIPE_BEGIN|pweight_active
clear
input long obs_id double(y sep pw fe)
1 1 0 0.5 1
2 0 0 3 1
3 0 1 7 1
4 2 0 2 2
5 0 0 5 2
6 0 1 11 2
end
assert _N == 6
`feppml_test_command' y c.sep [pw=pw], absorb(fe) separation(ir) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 2
assert e(N_physical) == 4
generate byte __p7_expected_drop = inlist(obs_id, 3, 6)
assert e(sample) == !__p7_expected_drop
assert __p7_case_drop == 11 if __p7_expected_drop
assert __p7_case_drop == 0 if !__p7_expected_drop
display as result "PHASE7_ORACLE|pweight_active|pweight_separation"
* PHASE7_RECIPE_END|pweight_active

* PHASE7_RECIPE_BEGIN|exposure_active
clear
input long obs_id double(y sep exposure fe)
1 1 0 1 1
2 0 0 2 1
3 0 1 3 1
4 2 0 4 2
5 0 0 5 2
6 0 1 6 2
end
assert _N == 6
`feppml_test_command' y c.sep, absorb(fe) exposure(exposure) separation(ir) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 2
assert e(N_physical) == 4
generate byte __p7_expected_drop = inlist(obs_id, 3, 6)
assert e(sample) == !__p7_expected_drop
assert __p7_case_drop == 11 if __p7_expected_drop
assert __p7_case_drop == 0 if !__p7_expected_drop
display as result "PHASE7_ORACLE|exposure_active|exposure_separation"
* PHASE7_RECIPE_END|exposure_active

* PHASE7_RECIPE_BEGIN|fweight_singleton_multiplicity
clear
input long obs_id double(y x fe fw)
1 6 0 1 2
2 2 3 2 1
3 7 -1 2 1
4 3 2 2 1
5 8 -2 2 1
6 4 1 3 1
7 9 -3 3 1
8 5 0 3 1
9 1 3 3 1
end
assert _N == 9
`feppml_test_command' y c.x [fw=fw], absorb(fe) separation(fe simplex ir) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 0
assert e(N_physical) == 9
assert e(sample)
assert __p7_case_drop == 0
assert e(num_singletons) == 0
assert e(N_physical) == 9
assert e(sample) if obs_id == 1
display as result "PHASE7_ORACLE|fweight_singleton_multiplicity|fweight_singleton_multiplicity"
* PHASE7_RECIPE_END|fweight_singleton_multiplicity

* PHASE7_RECIPE_BEGIN|zero_column_degenerate
clear
input long obs_id double(y zero)
1 1 0
2 0 0
3 0 0
end
assert _N == 3
`feppml_test_command' y c.zero, separation(simplex) dropreason(__p7_case_drop)
assert e(converged) == 1
assert e(separated) == 0
assert e(N_physical) == 3
assert e(sample)
assert __p7_case_drop == 0
display as result "PHASE7_ORACLE|zero_column_degenerate|zero_column_degenerate"
* PHASE7_RECIPE_END|zero_column_degenerate

* PHASE7_RECIPE_BEGIN|all_zero_terminal_failure
clear
input long obs_id double(y)
1 0
2 0
3 0
end
assert _N == 3
quietly regress y
tempname __p7_before_b __p7_before_v
matrix `__p7_before_b' = e(b)
matrix `__p7_before_v' = e(V)
quietly datasignature set, reset
capture noisily `feppml_test_command' y, separation(simplex)
assert _rc == 2001
quietly datasignature confirm
assert mreldif(e(b), `__p7_before_b') == 0
assert mreldif(e(V), `__p7_before_v') == 0
display as result "PHASE7_ORACLE|all_zero_terminal_failure|all_zero_failure"
* PHASE7_RECIPE_END|all_zero_terminal_failure

* Same-data exact determinism across thread counts.
clear
set obs 18
generate long obs_id = _n
generate byte fe = ceil(_n / 6)
generate byte separated_cell = fe == 3
generate double x = mod(5 * _n, 9) - 4
generate double y = 1 + mod(7 * _n, 10)
replace y = 0 if separated_cell
generate double __p7_pw = .5 + mod(_n, 7)
generate double __p7_exposure = exp(.02 * (mod(_n, 5) - 2))
`feppml_test_command' y c.x c.separated_cell [pw=__p7_pw], absorb(fe) ///
    exposure(__p7_exposure) separation(fe simplex ir) ///
    dropreason(__p7_t1_drop) threads(1) deterministic
tempname __p7_t1_b __p7_t1_v __p7_t1_receipt
matrix `__p7_t1_b' = e(b)
matrix `__p7_t1_v' = e(V)
matrix `__p7_t1_receipt' = e(feppml_receipt)
generate byte __p7_t1_sample = e(sample)
`feppml_test_command' y c.x c.separated_cell [pw=__p7_pw], absorb(fe) ///
    exposure(__p7_exposure) separation(fe simplex ir) ///
    dropreason(__p7_t4_drop) threads(4) deterministic
assert mreldif(`__p7_t1_b', e(b)) == 0
assert mreldif(`__p7_t1_v', e(V)) == 0
assert __p7_t1_sample == e(sample)
assert __p7_t1_drop == __p7_t4_drop
forvalues column = 25/32 {
    assert `__p7_t1_receipt'[1,`column'] == e(feppml_receipt)[1,`column']
}
display as result "PHASE7_DETERMINISM|threads=1,4|rc=0"

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_separation_methods|rc=0"
