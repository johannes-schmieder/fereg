version 18.0
clear all
set more off

* Summing all level-specific slope channels recovers the raw slope source,
* independently of its categorical partition. Thus 2^48*(x1-x2)=-zero is an
* exact separator in every specification below, including different,
* overlapping partitions. Syntax and level labels cannot erase that span.
set obs 48
generate byte g = 1 + floor((_n - 1) / 12)
generate byte h = 5 - g
generate byte a = mod(g - 1, 2)
generate byte b = floor((g - 1) / 2)
generate byte overlap = 1 + mod(floor((_n - 1) / 3), 2)
generate byte zero = mod(_n, 2)
generate double y = cond(zero, 0, 1 + mod(_n, 3))
generate double x1 = 1 + mod(_n, 3)
generate double x2 = x1 + 2^(-48) * zero
assert 2^48 * (x1 - x2) == -zero

foreach shape in joined split relabeled interaction overlap weighted {
    local absorb "g##c.(x1 x2)"
    if "`shape'" == "split" local absorb "g g#c.x1 g#c.x2"
    if "`shape'" == "relabeled" local absorb "g g#c.x1 h#c.x2"
    if "`shape'" == "interaction" local absorb "g g#c.x1 a#b#c.x2"
    if "`shape'" == "overlap" local absorb "g g#c.x1 overlap#c.x2"
    if "`shape'" == "weighted" {
        * This dependence uses level-specific coefficients; the two global
        * raw slope sources themselves are not nearly proportional.
        quietly replace x1 = (1 + mod(_n, 3)) / 16
        quietly replace x2 = (2^g * x1 + 2^(-48) * zero) / 2^overlap
        assert 2^48 * (2^g * x1 - 2^overlap * x2) == -zero
        local absorb "g g#c.x1 overlap#c.x2"
    }
    quietly feppml y if !zero, absorb(g) threads(1)
    matrix previous_b = e(b)
    matrix previous_V = e(V)
    generate byte previous_sample = e(sample)
    capture noisily feppml y, absorb(`absorb') separation(ir) threads(1) eta(proposed_eta)
    local result = _rc
    if `result' == 0 {
        assert e(N) == 24
        assert e(sample) == !zero
        assert missing(proposed_eta) if zero
        drop proposed_eta
    }
    else {
        assert inlist(`result', 430, 459)
        assert e(sample) == previous_sample
        assert mreldif(e(b), previous_b) == 0
        assert mreldif(e(V), previous_V) == 0
        confirm new variable proposed_eta
    }
    drop previous_sample
}

* A full-rank triangular FE basis can have tiny individual dual moments.
* Row-pivot elimination leaves this integer basis unchanged, but its inverse
* grows exponentially. The exact separator below isolates pattern zero.
clear
set obs 72
generate byte pattern = floor((_n - 1) / 2)
generate byte g = 1
generate double y = cond(pattern == 0, 0, .5 + mod(_n, 2))
generate double direction = -1
local slopes ""
forvalues j = 1/35 {
    generate double s`j' = cond(pattern < `j', 0, cond(pattern == `j', -1, 1))
    quietly replace direction = direction - 2^(`j' - 1) * s`j'
    local slopes "`slopes' s`j'"
}
assert direction == cond(pattern == 0, -1, 0)
quietly feppml y if pattern > 0, absorb(g) threads(1)
matrix previous_b = e(b)
matrix previous_V = e(V)
generate byte previous_sample = e(sample)
capture noisily feppml y, absorb(g##c.(`slopes')) separation(ir) threads(1) eta(proposed_eta)
local result = _rc
if `result' == 0 {
    assert e(N) == 70
    assert e(sample) == (pattern > 0)
    assert missing(proposed_eta) if pattern == 0
}
else {
    assert inlist(`result', 430, 459)
    assert e(sample) == previous_sample
    assert mreldif(e(b), previous_b) == 0
    assert mreldif(e(V), previous_V) == 0
    confirm new variable proposed_eta
}

display as result "END_FIXTURE|stability061_ppml_partition_contrasts|rc=0"
