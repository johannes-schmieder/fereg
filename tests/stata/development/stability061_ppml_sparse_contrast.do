version 18.0
clear all
set more off

* x - 1[g=2] - 1[g=3] is exactly -1 on rows6/7, zero elsewhere.
* Dropping those zeros makes row5 a singleton. The unit-metric IR candidate
* must pass the complete publication wire, including registered note codes.
input double(y x g h)
1 0 1 1
1 0 1 2
1 1 2 1
1 1 2 2
1 1 3 1
0 0 3 1
0 0 3 2
0 0 1 1
end
foreach shape in explicit absorbed {
    local rhs x
    local absvars "g h"
    if "`shape'" == "absorbed" {
        local rhs ""
        local absvars "g h x"
    }
    feppml y `rhs', absorb(`absvars') separation(ir) threads(1) eta(certified_eta)
    assert e(N) == 5
    assert e(sample) == inlist(_n, 1, 2, 3, 4, 8)
    assert missing(certified_eta) if !e(sample)
    drop certified_eta
}
display "END_FIXTURE|stability061_ppml_sparse_contrast|rc=0"
