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
display as result "END_FIXTURE|feppml_separation_recovery|rc=0"
