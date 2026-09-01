version 18.0
clear all
set more off

findfile _fereg_estimate_ppml.ado
run `"`r(fn)'"'

set obs 10
generate double y = _n

tempname b V status fit dof fe cluster subset separation
tempname inference drop_counts separation_sequence certificate runtime receipt bad badV
matrix `b' = (0, 0)
matrix `V' = I(2)
matrix `status' = (1,1,1,0 \ 2,1,1,0)

* A coherent, unweighted, no-FE fit with robust covariance.  The finite rank
* includes the structural constant and the robust correction is N/(N-1).
matrix `fit' = J(27,1,0)
matrix `fit'[1,1] = 10
matrix `fit'[2,1] = 2
matrix `fit'[4,1] = 8
matrix `fit'[5,1] = 8
matrix `fit'[6,1] = 1
matrix `fit'[7,1] = -5
matrix `fit'[8,1] = -6
matrix `fit'[9,1] = .1
matrix `fit'[10,1] = 1
matrix `fit'[11,1] = 2
matrix `fit'[12,1] = 1
matrix `fit'[13,1] = 10
matrix `fit'[14,1] = 2
matrix `fit'[15,1] = 3
matrix `fit'[17,1] = 10
matrix `fit'[18,1] = 10
matrix `fit'[19,1] = .
matrix `fit'[20,1] = 1
matrix `fit'[22,1] = 10
matrix `fit'[23,1] = 10
matrix `fit'[24,1] = 1
matrix `fit'[25,1] = .
matrix `fit'[26,1] = 0
matrix `fit'[27,1] = 1
matrix `dof' = (10 \ 2 \ 0 \ 2 \ 8 \ 8 \ . \ 0 \ 0 \ 10 \ 0 \ 0)
matrix `fe' = (2,0,0,0,0,0,0,0)
matrix `cluster' = J(1,4,0)
matrix `subset' = J(1,4,0)

* Singleton pruning is registered but executes zero times.  There are no
* statistical separation drops in this positive fixture.
matrix `separation' = (1,1,0,0,0,0,0,1,.,.,.,.,0,0)
matrix `inference' = J(16,1,0)
matrix `inference'[1,1] = 2
matrix `inference'[2,1] = 2
matrix `inference'[3,1] = 10 / 9
matrix `inference'[4,1] = 1
matrix `inference'[5,1] = 1
matrix `inference'[6,1] = 1
matrix `inference'[9,1] = 1
matrix `inference'[10,1] = 1
matrix `inference'[11,1] = 2
matrix `inference'[15,1] = 1e-8
matrix `inference'[16,1] = 1

matrix `drop_counts' = J(14,2,0)
forvalues row = 1/14 {
    matrix `drop_counts'[`row',1] = `row' - 1
}
matrix `drop_counts'[1,2] = 10

matrix `separation_sequence' = J(5,13,0)
forvalues row = 1/5 {
    matrix `separation_sequence'[`row',1] = `row'
    matrix `separation_sequence'[`row',13] = 1
}
matrix `separation_sequence'[1,2] = 1

matrix `certificate' = J(16,1,0)
matrix `certificate'[16,1] = 1
matrix `runtime' = J(108,1,.)
matrix `runtime'[1,1] = .1
matrix `runtime'[2,1] = 2
matrix `runtime'[3,1] = 3
matrix `runtime'[4,1] = 0
matrix `runtime'[5,1] = 0
matrix `runtime'[6,1] = 1
matrix `runtime'[7,1] = 2
matrix `runtime'[8,1] = 0
matrix `runtime'[9,1] = 0
matrix `runtime'[10,1] = 0
matrix `runtime'[11,1] = 1
forvalues row = 12/16 {
    matrix `runtime'[`row',1] = 0
}
forvalues row = 24/27 {
    matrix `runtime'[`row',1] = 0
}
matrix `runtime'[41,1] = 0
matrix `runtime'[77,1] = 1
matrix `runtime'[78,1] = 1
forvalues row = 83/95 {
    matrix `runtime'[`row',1] = 0
}

matrix `receipt' = J(1,33,0)
matrix `receipt'[1,1] = 5354602
matrix `receipt'[1,2] = 1
matrix `receipt'[1,3] = 1
matrix `receipt'[1,4] = 2
matrix `receipt'[1,5] = 2
matrix `receipt'[1,6] = 2
matrix `receipt'[1,7] = 10
matrix `receipt'[1,8] = 10
matrix `receipt'[1,9] = 10
matrix `receipt'[1,13] = 1
matrix `receipt'[1,15] = 1
matrix `receipt'[1,17] = 1
matrix `receipt'[1,20] = 16
matrix `receipt'[1,21] = 14
matrix `receipt'[1,22] = 5
matrix `receipt'[1,33] = 1

local validator_options `"clusters(0) ferows(0) subsets(0) separations(1) residual(0) saved(0) adjustment(0) vcecode(2) requestedvcecode(2) weightcode(0) memorycode(0) requestedthreads(0) feterms(0) sources(0) methodcodes("1") dropsingletons(1)"'

* The validator must not create or overwrite process-global scratch objects.
scalar __fereg_ppml_finite_result = 12345
mata: __fereg_ppml_fit = 67890
_fereg_eppml_validate `b' `V' `status' `fit' `dof' `fe' `cluster' ///
    `subset' `separation' `inference' `drop_counts' `separation_sequence' ///
    `certificate' `runtime' `receipt', capacity(2) `validator_options'
assert r(coefficients) == 2
assert scalar(__fereg_ppml_finite_result) == 12345
mata: assert(__fereg_ppml_fit == 67890)

capture noisily _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
    `separation_sequence' `certificate' `runtime' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 0
assert r(coefficients) == 2

* Every result-1.1 matrix is dimension-bound.  Structural mismatches and the
* receipt header/COMPLETE marker are protocol r(499), never semantic errors.
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
    `separation_sequence' `certificate' `runtime' `receipt', ///
    capacity(3) `validator_options'
assert _rc == 499

matrix `bad' = J(15,1,0)
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `bad' `drop_counts' ///
    `separation_sequence' `certificate' `runtime' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 499

matrix `bad' = J(13,2,0)
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `inference' `bad' ///
    `separation_sequence' `certificate' `runtime' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 499

matrix `bad' = J(5,12,0)
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `inference' `drop_counts' `bad' ///
    `certificate' `runtime' `receipt', capacity(2) `validator_options'
assert _rc == 499

matrix `bad' = J(1,32,0)
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
    `separation_sequence' `certificate' `runtime' `bad', ///
    capacity(2) `validator_options'
assert _rc == 499

foreach wiremutation in magic major minor inference_rows drop_rows sequence_rows complete {
    matrix `bad' = `receipt'
    if `"`wiremutation'"' == "magic" matrix `bad'[1,1] = 0
    if `"`wiremutation'"' == "major" matrix `bad'[1,2] = 2
    if `"`wiremutation'"' == "minor" matrix `bad'[1,3] = 0
    if `"`wiremutation'"' == "inference_rows" matrix `bad'[1,20] = 15
    if `"`wiremutation'"' == "drop_rows" matrix `bad'[1,21] = 13
    if `"`wiremutation'"' == "sequence_rows" matrix `bad'[1,22] = 4
    if `"`wiremutation'"' == "complete" matrix `bad'[1,33] = 0
    capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
        `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
        `separation_sequence' `certificate' `runtime' `bad', ///
        capacity(2) `validator_options'
    assert _rc == 499
}

* Result-1.1 evidence is semantic and therefore fails with internal r(3499)
* after the wire layout itself has been accepted.
foreach mutation in requested correction pass {
    matrix `bad' = `inference'
    if `"`mutation'"' == "requested" matrix `bad'[1,1] = 1
    if `"`mutation'"' == "correction" matrix `bad'[3,1] = 1
    if `"`mutation'"' == "pass" matrix `bad'[16,1] = 0
    capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
        `fe' `cluster' `subset' `separation' `bad' `drop_counts' ///
        `separation_sequence' `certificate' `runtime' `receipt', ///
        capacity(2) `validator_options'
    assert _rc == 3499
}

* Final PSD evidence uses the published covariance maximum-entry scale.  The
* exact negative tolerance boundary is accepted; a value below it is not.
matrix `bad' = `inference'
matrix `bad'[10,1] = -1e-8
_fereg_eppml_validate `b' `V' `status' `fit' `dof' `fe' `cluster' ///
    `subset' `separation' `bad' `drop_counts' `separation_sequence' ///
    `certificate' `runtime' `receipt', capacity(2) `validator_options'

matrix `bad'[10,1] = -1.0000001e-8
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `bad' `drop_counts' ///
    `separation_sequence' `certificate' `runtime' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 3499
assert scalar(__fereg_ppml_finite_result) == 12345
mata: assert(__fereg_ppml_fit == 67890)

* A row-sum norm would incorrectly admit this report: max(abs(V)) is one,
* while the largest absolute row sum is 1.5.
matrix `badV' = (1,.5 \ .5,1)
matrix `bad' = `inference'
matrix `bad'[10,1] = -1.25e-8
capture quietly _fereg_eppml_validate `b' `badV' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `bad' `drop_counts' ///
    `separation_sequence' `certificate' `runtime' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 3499

foreach mutation in reason retained negative {
    matrix `bad' = `drop_counts'
    if `"`mutation'"' == "reason" matrix `bad'[2,1] = 99
    if `"`mutation'"' == "retained" matrix `bad'[1,2] = 9
    if `"`mutation'"' == "negative" matrix `bad'[2,2] = -1
    capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
        `fe' `cluster' `subset' `separation' `inference' `bad' ///
        `separation_sequence' `certificate' `runtime' `receipt', ///
        capacity(2) `validator_options'
    assert _rc == 3499
}

foreach mutation in method requested execution certificate {
    matrix `bad' = `separation_sequence'
    if `"`mutation'"' == "method" matrix `bad'[1,1] = 2
    if `"`mutation'"' == "requested" matrix `bad'[1,2] = 0
    if `"`mutation'"' == "execution" matrix `bad'[1,4] = 1
    if `"`mutation'"' == "certificate" matrix `bad'[1,13] = 0
    capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
        `fe' `cluster' `subset' `separation' `inference' `drop_counts' `bad' ///
        `certificate' `runtime' `receipt', capacity(2) `validator_options'
    assert _rc == 3499
}

foreach mutation in executions generation_bound execution_bound fractional_hash oversized_hash {
    matrix `bad' = `receipt'
    if `"`mutation'"' == "executions" matrix `bad'[1,23] = 1
    if `"`mutation'"' == "generation_bound" matrix `bad'[1,24] = 11
    if `"`mutation'"' == "execution_bound" matrix `bad'[1,23] = 56
    if `"`mutation'"' == "fractional_hash" matrix `bad'[1,25] = .5
    if `"`mutation'"' == "oversized_hash" matrix `bad'[1,25] = 4294967296
    capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
        `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
        `separation_sequence' `certificate' `runtime' `bad', ///
        capacity(2) `validator_options'
    assert _rc == 3499
}

* The fixed sequence and bounded aggregate are two views of the same ledger.
matrix `bad' = `separation'
matrix `bad'[1,3] = 1
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `bad' `inference' `drop_counts' ///
    `separation_sequence' `certificate' `runtime' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 3499

* A failed developer allocation audit remains publishable telemetry.  Equal
* public linear-capacity summaries do not prove the private fingerprint bit.
matrix `runtime'[41,1] = 1
matrix `runtime'[42,1] = 16
matrix `runtime'[43,1] = 100
matrix `runtime'[44,1] = 101
matrix `runtime'[45,1] = 1
matrix `runtime'[46,1] = 1000
matrix `runtime'[47,1] = 1000
matrix `runtime'[48,1] = 0
matrix `runtime'[49,1] = 1200
matrix `runtime'[50,1] = 1200
matrix `runtime'[51,1] = 0
matrix `runtime'[52,1] = 10
matrix `runtime'[53,1] = 2
matrix `runtime'[54,1] = 5
matrix `runtime'[55,1] = 10
matrix `runtime'[56,1] = 2
matrix `runtime'[57,1] = 5
matrix `runtime'[58,1] = 1
matrix `runtime'[59,1] = 20
matrix `runtime'[60,1] = 10
matrix `runtime'[61,1] = 0
matrix `runtime'[62,1] = 100
matrix `runtime'[63,1] = 20
matrix `runtime'[64,1] = 10
matrix `runtime'[65,1] = 0
matrix `runtime'[66,1] = 100
matrix `runtime'[67,1] = 0
matrix `runtime'[68,1] = 0
matrix `runtime'[69,1] = 0
matrix `runtime'[70,1] = 1
matrix `runtime'[71,1] = 2
matrix `runtime'[72,1] = 1
matrix `runtime'[73,1] = 1
matrix `runtime'[74,1] = 1
matrix `runtime'[75,1] = 100
matrix `runtime'[76,1] = 0
_fereg_eppml_validate `b' `V' `status' `fit' `dof' `fe' `cluster' ///
    `subset' `separation' `inference' `drop_counts' `separation_sequence' ///
    `certificate' `runtime' `receipt', capacity(2) `validator_options'

* The fully passed audit, repeated-call identity, and complete action-event
* triple are accepted together.
matrix `bad' = `runtime'
matrix `bad'[12,1] = 100
matrix `bad'[13,1] = 80
matrix `bad'[14,1] = 100
matrix `bad'[17,1] = 100
matrix `bad'[18,1] = 120
matrix `bad'[19,1] = 120
matrix `bad'[20,1] = 130
matrix `bad'[21,1] = 10
matrix `bad'[22,1] = 0
matrix `bad'[23,1] = .1
matrix `bad'[44,1] = 100
matrix `bad'[45,1] = 0
matrix `bad'[67,1] = 1
matrix `bad'[76,1] = 1
matrix `bad'[79,1] = 20
matrix `bad'[80,1] = 10
matrix `bad'[81,1] = 12
matrix `bad'[82,1] = 2
matrix `bad'[25,1] = 1
_fereg_eppml_validate `b' `V' `status' `fit' `dof' `fe' `cluster' ///
    `subset' `separation' `inference' `drop_counts' `separation_sequence' ///
    `certificate' `bad' `receipt', capacity(2) `validator_options'

foreach mutation in route qualified certificate scratch modeled auditpartial {
    matrix `bad' = `runtime'
    if `"`mutation'"' == "route" matrix `bad'[71,1] = 3
    if `"`mutation'"' == "qualified" matrix `bad'[72,1] = 0
    if `"`mutation'"' == "certificate" matrix `bad'[76,1] = 1
    if `"`mutation'"' == "scratch" matrix `bad'[69,1] = 1
    if `"`mutation'"' == "modeled" matrix `bad'[83,1] = .
    if `"`mutation'"' == "auditpartial" matrix `bad'[42,1] = .
    capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
        `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
        `separation_sequence' `certificate' `bad' `receipt', ///
        capacity(2) `validator_options'
    assert _rc == 3499
}

matrix `bad' = `runtime'
matrix `bad'[17,1] = 100
matrix `bad'[18,1] = 120
matrix `bad'[79,1] = 19
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
    `separation_sequence' `certificate' `bad' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 3499

matrix `bad' = `runtime'
matrix `bad'[80,1] = 10
capture quietly _fereg_eppml_validate `b' `V' `status' `fit' `dof' ///
    `fe' `cluster' `subset' `separation' `inference' `drop_counts' ///
    `separation_sequence' `certificate' `bad' `receipt', ///
    capacity(2) `validator_options'
assert _rc == 3499

scalar drop __fereg_ppml_finite_result
mata: mata drop __fereg_ppml_fit

display as result "END_FIXTURE|estimate_ppml_runtime_validator|rc=0"
