version 18
args expected_rc
clear all
set more off
set obs 192
generate long row=_n
generate byte g=1+mod(_n,4)
generate byte h=1+mod(floor((_n-1)/4),3)
generate byte cl=1+mod(floor((_n-1)/12),8)
generate double w=1+mod(_n,5)
generate double z=sin(_n*.371)
generate double x=cos(_n*.619)
generate double duplicate=x
generate double y=10+.4*g+.2*h*z+.7*x+.05*sin(_n*1.337)
local fits=0
forvalues case=1/4 {
    preserve
    local weight
    local vce
    local mode fast
    local finite z x
    local oracle x i.g
    local absorb g
    local saved saved_g=g
    if `case'==1 quietly replace z=g
    if `case'==2 {
        local weight [aw=w]
        local vce vce(robust)
        local oracle x i.g ibn.h#c.z
        local absorb g h#c.z
        local saved saved_g=g h#c.z
    }
    if `case'>=3 {
        local finite x duplicate
        local vce vce(cluster cl)
        if `case'==3 {
            local weight [fw=w]
            local mode low
        }
        if `case'==4 local weight [pw=w]
    }
    quietly regress y `oracle' if mod(row,11)!=0 `weight', `vce'
    matrix oracle_b=e(b)
    matrix oracle_v=e(V)
    scalar oracle_df=e(df_r)
    generate byte oracle_sample=e(sample)
    predict double oracle_e if oracle_sample, residuals
    quietly fereg y `finite' if mod(row,11)!=0 `weight', absorb(`absorb') `vce' memory(`mode') threads(4)
    assert e(sample)==oracle_sample
    assert e(df_r)==oracle_df
    matrix before_b=e(b)
    matrix before_v=e(V)
    matrix before_status=e(fereg_status)
    local xindex=colnumb(before_b,"x")
    scalar fitted_x=_b[x]
    scalar fitted_v=before_v[`xindex',`xindex']
    if `case'>=3 {
        local dindex=colnumb(before_b,"duplicate")
        scalar fitted_x=fitted_x+_b[duplicate]
        scalar fitted_v=fitted_v+2*before_v[`xindex',`dindex']+before_v[`dindex',`dindex']
    }
    else {
        assert _b[z]==0
        assert before_status[1,2]==3
    }
    assert abs(fitted_x-oracle_b[1,1])<1e-10
    assert abs(fitted_v-oracle_v[1,1])<1e-10
    capture noisily fereg y `finite' if mod(row,11)!=0 `weight', absorb(`saved') `vce' memory(`mode') threads(4) residuals(saved_e)
    local actual_rc=_rc
    assert `actual_rc'==`expected_rc'
    assert mreldif(e(b),before_b)<1e-12
    assert mreldif(e(V),before_v)<1e-12
    assert mreldif(e(fereg_status),before_status)==0
    assert e(sample)==oracle_sample
    assert row==_n
    if `actual_rc'==0 {
        assert abs(saved_e-oracle_e)<=1e-9+1e-7*abs(oracle_e) if oracle_sample
        assert missing(saved_g) & missing(saved_e) if !oracle_sample
        quietly levelsof g, local(groups)
        foreach group of local groups {
            quietly summarize saved_g if oracle_sample & g==`group', meanonly
            assert r(min)==r(max)
        }
    }
    else {
        capture confirm variable saved_g
        assert _rc==111
        capture confirm variable saved_e
        assert _rc==111
    }
    local fits=`fits'+1
    restore
}
display "OMITTED_OUTPUTS_ORACLE|fits=" `fits' "|expected_rc=`expected_rc'|rc=0"
