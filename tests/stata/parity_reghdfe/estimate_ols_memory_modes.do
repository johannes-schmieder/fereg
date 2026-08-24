version 18.0
clear all
set more off
set seed 24082026
set obs 10000

generate long id = ceil(_n / 10)
generate double y = rnormal()
forvalues column = 1/20 {
    generate double x`column' = rnormal() + id / (`column' + 10)
    replace y = y + x`column' * (`column' / 200)
}

local constrained : environment FEREG_MEMORY_LIMIT_BYTES
if `"`constrained'"' == "" {
    fereg y x1-x20, absorb(id) memory(fast) poolsize(1)
    matrix b_fast = e(b)
    matrix V_fast = e(V)
    assert `"`e(fereg_memory_requested)'"' == "fast"
    assert `"`e(fereg_memory_selected)'"' == "fast"
    assert e(fereg_runtime)[1,2] == 1
    assert e(fereg_runtime)[2,2] == 1
    assert e(fereg_runtime)[6,1] == 1
    assert e(fereg_runtime)[9,2] == 0
    assert e(fereg_runtime)[13,2] > 0
    assert e(fereg_runtime)[21,2] >= 0

    fereg y x1-x20, absorb(id) memory(low) poolsize(1)
    assert mreldif(e(b), b_fast) <= 1e-10
    assert mreldif(e(V), V_fast) <= 1e-10
    assert `"`e(fereg_memory_requested)'"' == "low"
    assert `"`e(fereg_memory_selected)'"' == "low"
    assert e(fereg_runtime)[1,2] == 2
    assert e(fereg_runtime)[2,2] == 2
    assert e(fereg_runtime)[3,2] == 1
    assert e(fereg_runtime)[9,2] > 0
    assert e(fereg_runtime)[13,2] > 0
    assert e(fereg_runtime)[4,2] > e(fereg_runtime)[5,2]

    fereg y x1-x20, absorb(id) memory(auto) poolsize(1)
    assert mreldif(e(b), b_fast) <= 1e-10
    assert mreldif(e(V), V_fast) <= 1e-10
    assert `"`e(fereg_memory_requested)'"' == "auto"
    assert `"`e(fereg_memory_selected)'"' == "fast"
    assert e(fereg_runtime)[1,2] == 3
    assert e(fereg_runtime)[2,2] == 1
}
else {
    capture noisily fereg y x1-x20, absorb(id) memory(fast) poolsize(1)
    assert _rc == 909

    fereg y x1-x20, absorb(id) memory(low) poolsize(1)
    matrix b_low = e(b)
    matrix V_low = e(V)
    local fast_bytes = e(fereg_runtime)[4,2]
    local low_bytes = e(fereg_runtime)[5,2]
    assert e(fereg_runtime)[6,2] == real(`"`constrained'"')
    assert e(fereg_runtime)[8,2] >= `low_bytes'
    assert e(fereg_runtime)[8,2] < `fast_bytes'

    fereg y x1-x20, absorb(id) memory(auto) poolsize(1)
    assert mreldif(e(b), b_low) <= 1e-10
    assert mreldif(e(V), V_low) <= 1e-10
    assert `"`e(fereg_memory_requested)'"' == "auto"
    assert `"`e(fereg_memory_selected)'"' == "low"
    assert e(fereg_runtime)[1,2] == 3
    assert e(fereg_runtime)[2,2] == 2
    assert e(fereg_runtime)[9,2] > 0
}

di as result "END_FIXTURE|estimate_ols_memory_modes|rc=0"
