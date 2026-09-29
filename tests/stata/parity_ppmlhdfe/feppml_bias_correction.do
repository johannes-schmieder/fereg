version 18.0
clear all
set more off
set obs 12
generate long row = _n
generate byte g = ceil(_n/2)
generate byte h = 1+mod(_n-1,2)
generate double x = real(word("-1.5 -.5 -1 .5 -.5 1 0 1.5 .5 2 1 2.5",_n))
generate double y = real(word("1 2 2 4 3 5 4 7 5 8 6 9",_n))
generate double w = real(word("1 2 1.5 .5 3 2 4 1 2.5 3.5 1 2",_n))
generate double off = real(word("0 .05 .1 0 -.05 .1 0 .05 -.1 0 .1 .05",_n))
generate double unit = row

quietly feppml y x [pw=w], absorb(ordinary_g=g ordinary_h=h) offset(off) mu(ordinary_mu) keepsingletons
matrix ordinary_b=e(b)
matrix ordinary_v=e(V)
local random_state `"`c(rngstate)'"'
quietly feppml y x [pw=w], absorb(exact_g=g exact_h=h) offset(off) biascorrection(exact) biasunit(unit) biasfe(bf) mu(exact_mu) keepsingletons
assert `"`c(rngstate)'"'==`"`random_state'"'
assert mreldif(ordinary_b,e(b))==0
assert mreldif(ordinary_v,e(V))==0
assert ordinary_mu==exact_mu
assert ordinary_g==exact_g
assert ordinary_h==exact_h
assert abs(e(b_bias_d)[1,1] - (-.009115805294346302))<2e-8
assert abs(e(b_bias_c)[1,1] - (-.0001286809017592705))<2e-8
assert e(b_bias_total)[1,2]==0
assert e(b_bc)[1,2]==0
assert e(bias_diagnostics)[2,1]==12
assert e(bias_diagnostics)[7,1]==14
assert e(bias_mcse_total)[1,1]==0
assert bf_2_raw==0 if h==1
assert bf_2_d==0 if h==1
assert bf_2_c==0 if h==1
assert reldif(log(exact_mu)-off,bf_1_raw+bf_2_raw+e(b_bias_raw)[1,1]*x)<1e-8
assert reldif(bf_1_bc,bf_1_raw+bf_1_total)<1e-14
assert reldif(bf_2_bc,bf_2_raw+bf_2_total)<1e-14
matrix exact_b=e(b_bc)
matrix exact_support=e(bias_support)
assert e(bias_support)[1,1]==6
assert e(bias_support)[1,2]==2
assert e(bias_support)[2,2]==6
assert e(bias_support)[1,7]==0
assert e(bias_support)[2,8]==0
* Identical or relabelled FE IDs cannot supply independent within-level support.
generate double copied_unit=103-7*g
foreach id in g h copied_unit {
    foreach method in exact randomized {
        capture noisily feppml y x [pw=w], absorb(g h) offset(off) biascorrection(`method') biasunit(`id') keepsingletons
        assert _rc!=0
        assert mreldif(exact_b,e(b_bc))==0
        assert mreldif(exact_support,e(bias_support))==0
        capture noisily feppml y x [pw=w], absorb(g h) offset(off) biascorrection(`method') biasunit(`id') biasfe(unsupported) keepsingletons
        assert _rc!=0
        capture confirm variable unsupported_1_raw
        assert _rc==111
    }
}
* Only one FE level loses independent support: whole requested correction fails.
replace copied_unit=unit
replace copied_unit=1 if g==1
capture noisily feppml y x [pw=w], absorb(g h) offset(off) biascorrection(exact) biasunit(copied_unit) keepsingletons
assert _rc!=0
assert mreldif(exact_b,e(b_bc))==0
* A sample restriction can create unsupported levels even with valid full data.
capture noisily feppml y x [pw=w] if row!=3, absorb(g h) offset(off) biascorrection(exact) biasunit(unit) keepsingletons
assert _rc!=0
assert mreldif(exact_b,e(b_bc))==0
* Reversing unit labels leaves membership and exact results unchanged.
replace copied_unit=100-unit
quietly feppml y x [pw=w], absorb(g h) offset(off) biascorrection(exact) biasunit(copied_unit) keepsingletons
assert mreldif(exact_b,e(b_bc))<1e-12
assert mreldif(exact_support,e(bias_support))<1e-12
* Units can cross both FE partitions: no nesting restriction.
replace copied_unit=1+mod(row-1,3)
quietly feppml y x [pw=w], absorb(g h) offset(off) biascorrection(exact) biasunit(copied_unit) keepsingletons
assert e(bias_support)[1,2]==2
assert e(bias_support)[2,2]==3
assert mreldif(ordinary_b,e(b))==0
generate str8 gs="g"+string(g)
generate str8 hs="h"+string(h)
quietly feppml y x [pw=w], absorb(gs hs) offset(off) biascorrection(exact) biasunit(unit) keepsingletons
assert mreldif(exact_b,e(b_bc))<1e-12
quietly feppml y x [pw=w], absorb(g h) offset(off) biascorrection(randomized) biasunit(unit) biasprobes(2) biasseed(4294967295) biasstream(4294967295) keepsingletons
assert e(bias_diagnostics)[4,1]==4294967295
assert e(bias_diagnostics)[5,1]==4294967295
assert e(bias_diagnostics)[7,1]==6

quietly feppml y x [pw=w], absorb(g h) offset(off) biascorrection(randomized) biasunit(unit) biasprobes(256) biasseed(1021) biasstream(1) keepsingletons threads(1)
matrix random_b=e(b_bc)
matrix random_d=e(b_bias_d)
matrix random_c=e(b_bias_c)
matrix random_support=e(bias_support)
assert `"`c(rngstate)'"'==`"`random_state'"'
assert abs(e(bias_mcse_total)[1,1]^2-e(bias_mcse_d)[1,1]^2-e(bias_mcse_c)[1,1]^2-2*e(bias_cov_dc)[1,1])<1e-14
gsort -row
quietly feppml y x [pw=w], absorb(g h) offset(off) biascorrection(randomized) biasunit(unit) biasprobes(256) biasseed(1021) biasstream(1) keepsingletons threads(2) memory(low) poolsize(1)
assert mreldif(random_b,e(b_bc))<1e-8
assert mreldif(random_d,e(b_bias_d))<1e-8
assert mreldif(random_c,e(b_bias_c))<1e-8
assert mreldif(random_support,e(bias_support))<1e-8
quietly feppml y x [pw=w], absorb(g h) offset(off) biascorrection(randomized) biasunit(unit) biasprobes(256) biasseed(1021) biasstream(2) keepsingletons
assert mreldif(random_b,e(b_bc))>1e-10

* Failed requested corrections restore the previous e() and create no outputs.
matrix held=e(b_bc)
replace unit=. if row==3
capture noisily feppml y x [pw=w], absorb(g h) offset(off) biascorrection(exact) biasunit(unit) biasfe(fail) keepsingletons
assert _rc!=0
assert mreldif(held,e(b_bc))==0
capture confirm variable fail_1_raw
assert _rc==111
capture noisily feppml y x, absorb(g h) biascorrection(exact)
assert _rc==198
capture noisily feppml y x, absorb(g h) biascorrection(exact) biasunit(g) biasprobes(10)
assert _rc==198
capture noisily feppml y x, absorb(g h) biascorrection(randomized) biasunit(g) biasprobes(1)
assert _rc==198
capture noisily feppml y x, absorb(g) biascorrection(exact) biasunit(g)
assert _rc==198
quietly feppml y x [pw=w] if g!=2, absorb(g h) offset(off) biascorrection(exact) biasunit(unit) biasfe(excluded) keepsingletons
assert missing(excluded_1_bc) if g==2
assert excluded_1_bc<. if e(sample)
replace y=0 if g==1
capture noisily feppml y x, absorb(g h) biascorrection(exact) biasunit(g) biasfe(selected) keepsingletons
assert _rc!=0
capture confirm variable selected_1_raw
assert _rc==111
quietly feppml y x, absorb(g h) keepsingletons
assert e(N)==10
assert e(sample) if row==3
assert `"`e(bias_method)'"'==""
capture confirm matrix e(b_bc)
assert _rc!=0
display "END_FIXTURE|feppml_bias_correction|rc=0"
