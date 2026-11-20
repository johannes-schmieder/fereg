version 18
clear all
set more off

* Conditional on (t,h,k,l), the two positive corners have (x,g)=(0,1)
* and (1,2). The direction I(g=2)-x separates both copies of (1,1).
* Replacing one copy with (0,2) closes the four-corner identity. Equal
* signed masses then balance every column. The treatment doubles the
* conditional mean on both samples and is identified after deletion.
quietly set obs 403200
generate long copy = floor((_n-1)/4)
generate byte j = mod(_n-1,4)
generate byte g = 1+(j==1)
generate byte x = j!=0
generate byte t = mod(copy,2)
generate byte h = mod(floor(copy/2),7)
generate byte k = mod(floor(copy/14),5)
generate byte l = mod(floor(copy/70),3)
generate byte y = 2*(j<2)*(1+t)

* An explicit ceiling of one million cells fits the grouped ray but
* cannot replay its physical rows. It must still fail atomically.
quietly regress y t
matrix before = e(b)
capture quietly _fereg_estimate_ppml y x t, absorb(g h k l) threads(1) ///
    separation(simplex) simplexcells(1000000) mu(rejected) dropreason(rejected_reason)
assert _rc==909
assert "`e(cmd)'"=="regress" & mreldif(before,e(b))==0
confirm new variable rejected
confirm new variable rejected_reason

foreach side in open closed {
    local closed = "`side'"=="closed"
    if `closed' {
        replace x = 0 if j==3
        replace g = 2 if j==3
    }
    quietly feppml y x t, absorb(g h k l) threads(1) separation(simplex) ///
        mu(fit) dropreason(reason)
    generate byte retained = `closed' | j<2
    assert e(converged)==1 & e(num_singletons)==0
    assert e(sample)==retained & reason==cond(retained,0,10)
    assert e(separated)==cond(`closed',0,_N/2)
    assert abs(_b[t]-ln(2))<1e-7
    assert abs(fit-(1+t)*cond(`closed',1,2))<1e-7 if retained
    assert missing(fit) if !retained
    if `closed' {
        * The profiled treatment information is N/3 and its score
        * meat is 4N/9, giving the uncorrected robust variance 4/N.
        assert abs(_se[t]^2/e(feppml_vce_correction)*_N-4)<1e-7
        assert abs(e(deviance)/_N-3*ln(2))<1e-8
    }
    drop fit retained reason
}

* Each group's positive observations lie on x=a_g+sum(b_gj*z_j).
* Its last observation lies one unit below the plane. Subtracting the
* group-specific terms from x gives a single exact direction, zero
* on all positives and -1 on every displaced observation.
clear
set seed 161824
local slopes 32
local groups 1200
local positive = 2*(`slopes'+1)
quietly set obs `=`groups'*(`positive'+1)'
generate long g = floor((_n-1)/(`positive'+1))
generate byte slot = mod(_n-1,`positive'+1)
generate byte displaced = slot==`positive'
generate byte y = 2*!displaced
generate double x = mod(g,7)-displaced
local channels
forvalues q = 1/`slopes' {
    generate byte z`q' = runiformint(-8,8)+48*(mod(slot,`slopes'+1)==`q')
    replace x = x+(1+mod(g+`q',3))*z`q'
    local channels `channels' z`q'
}
quietly feppml y x, absorb(g##c.(`channels')) threads(1) separation(ir) ///
    mu(fit) dropreason(reason)
assert e(converged)==1 & e(num_singletons)==0 & e(separated)==`groups'
assert e(sample)==!displaced & reason==cond(displaced,11,0)
assert abs(fit-2)<1e-7 if !displaced
assert missing(fit) if displaced

* Each cell contains two positive endpoints and their zero midpoint.
* A displaced zero permits a strict score of -1 in the open case.
* An opposite displacement makes their average a second copy of the
* midpoint in the closed case. Both endpoints have outcome 2, so the
* shared fitted mean is 4/3 on the open sample and 4/5 on the closed one.
foreach side in open closed {
    clear
    set seed 220719
    quietly set obs 4096
    generate byte block = floor((_n-1)/1024)
    foreach v in a b c {
        generate int `v' = runiformint(1,128)
    }
    generate int g = a+128*block
    generate int h = b+128*block
    generate int k = c+128*block
    generate double center = (_n-1)/64
    generate byte slope = 1+mod(k,2)
    local closed = "`side'"=="closed"
    expand `=4+`closed''
    bysort center: generate byte j = _n-1
    generate double x = center+cond(j==0,-1,cond(j==1,1,0))
    generate double z = -(mod(a,7)+2*mod(b,5)+3*mod(c,3)+x+(j==3)-(j==4))/slope
    generate byte y = 2*(j<2)
    generate double perm = runiform()
    sort perm
    generate byte retained = `closed' | j!=3
    local methods ir
    if !`closed' local methods default ir
    foreach method of local methods {
        local option = cond("`method'"=="default","","separation(ir)")
        quietly feppml y x, absorb(g h k##c.z) threads(1) `option' mu(fit) dropreason(reason)
        assert e(converged)==1 & e(num_singletons)==0
        assert e(separated)==cond(`closed',0,4096) & e(sample)==retained
        assert reason==0 & abs(fit-cond(`closed',4/5,4/3))<1e-7 if retained
        assert inlist(reason,10,11) & missing(fit) if !retained
        if "`method'"=="ir" assert reason==11 if !retained
        drop fit reason
    }
}

display "END_FIXTURE|feppml_adaptive_sparse_support|rc=0"
