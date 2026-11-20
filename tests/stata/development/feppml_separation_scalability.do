version 18
set more off

* Each disconnected two-by-two table has positive outcomes on both diagonals.
* For every additive model direction, x00+x11=x01+x10. With both off-diagonals
* present, neither can separate. Removing 10 leaves precisely 01 separable.
* At 26 blocks and 96 copies the old positive-row scratch alone exceeded the
* public 1,000,000-cell cap. The 64-block repeated design also requires
* streaming and coalescing before allocating the boundary tableau.
local copies = 96
foreach design in repeated unique {
    local blocks = cond("`design'"=="repeated",64,26)
    clear
    local slots = cond("`design'"=="repeated",8,4)
    quietly set obs `=`blocks'*`copies'*`slots''
    generate byte slot = mod(_n-1,`slots')
    generate int copy = mod(floor((_n-1)/`slots'),`copies')
    generate int block = floor((_n-1)/(`slots'*`copies'))
    local extra

    if "`design'"=="repeated" {
        * Both diagonal patterns mix y=2 and y=0; the boundary multiplicities
        * of 01 and 10 are 3:1. Every physical zero needs positive dual weight.
        generate byte corner = cond(slot<2,0,cond(slot<4,3,cond(slot<7,1,2)))
        generate byte y = 2*inlist(slot,0,2)
        * Equal off-diagonal fitted totals m and diagonal totals 2-m solve
        * the margins. The log-linear restriction gives ((2-m)/2)^2=m^2/3.
        local mass = 2*sqrt(3)/(2+sqrt(3))
        generate double oracle = cond(inlist(corner,0,3),(2-`mass')/2, ///
            cond(corner==1,`mass'/3,`mass'))
    }
    else {
        generate byte corner = slot
        generate byte y = inlist(corner,0,3)
        * These seven regressors distinguish all 96 copies. They take the
        * same values on all four corners, preserving each four-row identity.
        forvalues bit = 0/6 {
            generate byte tag`bit' = mod(floor(copy/2^`bit'),2)
        }
        local extra tag0-tag6
        generate double oracle = 0.5
    }
    generate int left = 2*block + floor(corner/2)
    generate int right = 2*block + mod(corner,2)
    if "`design'"=="unique" {
        isid left right `extra'
    }
    quietly count if y>0
    assert 2*r(N)*(4*`blocks'-1)>1000000

    forvalues opened = 0/1 {
        if `opened' {
            drop if block==0 & corner==2
            replace oracle = 1 if block==0
            gsort -right copy -left slot
        }
        generate byte kept = !(`opened' & block==0 & corner==1)
        quietly count if !kept
        local separated = r(N)
        assert `separated'==`opened'*`copies'*cond("`design'"=="repeated",3,1)
        local use_default = ("`design'"=="repeated") != `opened'
        local sepopt separation(simplex)
        if `use_default' {
            local sepopt
        }
        quietly feppml y i.left i.right `extra', `sepopt' threads(1) ///
            mu(actual) dropreason(reason)
        assert e(N)==_N-`separated' & e(separated)==`separated'
        assert e(sample)==kept & reason==cond(kept,0,10)
        assert abs(actual-oracle)<1e-7 if kept
        assert missing(actual) if !kept
        assert e(feppml_separation_sequence)[3,9]==`separated'
        assert e(feppml_separation_sequence)[3,13]==1
        if `use_default' {
            * The terminal IR report reuses the certified simplex proof.
            assert e(feppml_separation_sequence)[4,7]==2
            assert e(feppml_separation_sequence)[4,8]==0
            assert e(feppml_separation_sequence)[4,13]==1
        }
        drop kept actual reason
    }
}

display as result "END_FIXTURE|feppml_separation_scalability|rc=0"
