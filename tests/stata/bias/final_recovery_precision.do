version 18.0
clear all
set more off
set obs 4096
generate long row = _n - 1
generate byte g = mod(row,8)
generate byte h = mod(floor(row/8),8)
generate double x = sin(row*.37)
generate double y = 2 + mod(row,5)
generate double exposure = .2 + mod(row,17)/8
replace exposure = 5e-7 in 38
generate long unit = floor(row/16)

foreach mode in auto low {
    quietly feppml y x, absorb(g h) exposure(exposure) keepsingletons ///
        memory(`mode') threads(2) mu(m0)
    matrix b0=e(b)
    matrix V0=e(V)
    local iterations=e(ic)
    quietly feppml y x, absorb(ag=g ah=h) exposure(exposure) keepsingletons ///
        memory(`mode') threads(2) mu(m1) d(ad)
    assert mreldif(b0,e(b))==0
    assert mreldif(V0,e(V))==0
    assert m0==m1
    assert e(ic)==`iterations'
    assert abs(ag+ah-ad)<1e-10
    quietly feppml y x, absorb(g h) exposure(exposure) keepsingletons ///
        memory(`mode') threads(2) mu(m2) biascorrection(exact) ///
        biasunit(unit) biasfe(bf)
    assert mreldif(b0,e(b))==0
    assert mreldif(V0,e(V))==0
    assert m0==m2
    assert e(ic)==`iterations'
    assert abs(log(m2)-log(exposure)-e(b_bias_raw)[1,1]*x-bf_1_raw-bf_2_raw)<1e-9
    assert bf_1_bc<. & bf_2_bc<.
    drop m0 m1 m2 ag ah ad bf_*
}
display "END_FIXTURE|final_recovery_precision|rc=0"
