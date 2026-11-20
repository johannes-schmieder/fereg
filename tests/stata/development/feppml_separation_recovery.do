version 18
set more off
clear
set obs 23
generate double y = max(_n-17,0)
generate double a = (_n<=2)-(_n==3)
generate double b = inrange(_n,4,6)-inrange(_n,7,8)
generate double c = cond(inrange(_n,9,17),_n-8,0)
replace c = 1000 in 17
generate byte retained = c==0
generate double eta = log(21/(6+2*sqrt(2)+2*sqrt(6))) - log(2)*a/2 + log(2/3)*b/2
generate double x1 = (a+c)/16
generate double x2 = 32*(b+c)
generate double x3 = (a+b+c)/8

foreach method in default simplex ir {
    quietly feppml y a b c, separation(`method') mu(prediction) dropreason(reason)
    assert e(N)==14 & e(sample)==retained
    assert abs(log(prediction)-eta)<1e-10 if retained
    assert missing(prediction) & inrange(reason,10,11) if !retained
    drop prediction reason

    gsort -b -a -c
    quietly feppml y x3 x2 x1, separation(`method') mu(prediction)
    assert e(N)==14 & e(sample)==retained
    assert abs(log(prediction)-eta)<1e-10 if retained
    drop prediction
}

keep if retained
quietly feppml y b a, separation(ir) mu(prediction)
assert e(N)==14 & e(sample)==1
assert abs(log(prediction)-eta)<1e-10

// Three imbalanced two-by-two tables share the specification. Positive
// outcomes alone lack three directions, but each pair of off-diagonals
// closes one direction. The dense simplex does not fit the default cap.
clear
set obs 18
generate byte block = floor((_n-1)/6)
generate byte cell = mod(_n-1,6)
generate byte left = 2*block + inlist(cell,1,5)
generate byte right = 2*block + inrange(cell,1,4)
generate double y = cell < 2
expand 96

quietly poisson y i.left i.right
quietly predict double reference, n
quietly feppml y i.left i.right, separation(simplex) mu(actual)
assert e(N)==_N & e(separated)==0
assert abs(actual-reference)<1e-7
drop actual reference

// Removing one off-diagonal leaves a one-way separation in just one table.
drop if block==0 & cell==5
generate byte retained = !(block==0 & inrange(cell,2,4))
quietly poisson y i.left i.right if retained
quietly predict double reference, n
gsort -right -left -y
quietly feppml y i.left i.right, separation(simplex) mu(actual)
assert e(sample)==retained & e(separated)==288
assert abs(actual-reference)<1e-7 if retained
assert missing(actual) if !retained

display as result "END_FIXTURE|feppml_separation_recovery|rc=0"
