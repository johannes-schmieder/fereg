version 17.0
clear all
set more off
set obs 32768
generate double x=cond(mod(_n-1,4)<2,-1,1)
generate double z=cond(mod(floor((_n-1)/4),2)==0,-1,1)
generate double u=cond(mod(_n,2)==0,.125,-.125)
generate double y=3+2*x-.5*z+u
program define _centered_marker
    args name
    local trace : environment FEREG_SOLVER_TRACE_FILE
    if `"`trace'"'!="" {
        file open centeredtrace using `"`trace'"', write append text
        file write centeredtrace "FEREG_SOLVER_TRACE|phase=no_fe_centered_case|name=`name'" _n
        file close centeredtrace
    }
end
program define _centered_check
    syntax anything [if] [in], NAME(string)
    quietly regress `anything' `if' `in'
    matrix wanted_b=e(b)
    matrix wanted_v=e(V)
    local wanted_rss=e(rss)
    local wanted_n=e(N)
    local wanted_rank=e(N)-e(df_r)
    foreach workers in 1 4 16 {
        _centered_marker `name'_`workers'
        quietly fereg `anything' `if' `in', threads(`workers')
        assert e(N)==`wanted_n' & e(N)-e(df_r)==`wanted_rank'
        assert mreldif(e(b),wanted_b)<1e-8
        assert mreldif(e(V),wanted_v)<1e-8
        assert abs(e(rss)-`wanted_rss')<1e-8*max(1,`wanted_rss')
        assert "`e(fe_residual_method)'"=="derived"
        matrix repeat_b=e(b)
        matrix repeat_v=e(V)
        scalar repeat_rss=e(rss)
        quietly fereg `anything' `if' `in', threads(`workers')
        assert mreldif(e(b),repeat_b)==0
        assert mreldif(e(V),repeat_v)==0
        assert e(rss)==scalar(repeat_rss)
    }
end
_centered_check y x z, name(ordinary)
generate double small=x*1e-100
generate double large=z*1e100
* At these scales regress omits small; the orthogonal design supplies an
* independent witness for the mathematically unchanged, full-rank model.
foreach workers in 1 4 16 {
    _centered_marker extreme_scale_`workers'
    quietly fereg y small large, threads(`workers')
    assert e(N)-e(df_r)==3
    assert abs(_b[small]*1e-100-2)<1e-10
    assert abs(_b[large]*1e100+.5)<1e-10
    assert abs(_b[_cons]-3)<1e-10
    assert abs(e(rss)-512)<1e-8
    matrix scaled_v=e(V)
    scalar wanted_variance=.015625/(_N-3)
    assert abs(scaled_v[1,1]*1e-200/wanted_variance-1)<1e-10
    assert abs(scaled_v[2,2]*1e200/wanted_variance-1)<1e-10
    assert abs(scaled_v[3,3]/wanted_variance-1)<1e-10
    assert "`e(fe_residual_method)'"=="derived"
    matrix repeat_b=e(b)
    matrix repeat_v=e(V)
    scalar repeat_rss=e(rss)
    quietly fereg y small large, threads(`workers')
    assert mreldif(e(b),repeat_b)==0 & mreldif(e(V),repeat_v)==0
    assert e(rss)==scalar(repeat_rss)
}
generate double exact=3+2*x-.5*z
_centered_check exact x z, name(exact_fit)
generate double zero=0
generate double constant=7
_centered_check zero x z, name(zero_outcome)
_centered_check constant x z, name(constant_outcome)
_centered_check y, name(intercept_only)
generate byte g=mod(_n,7)
generate byte h=mod(floor(_n/7),3)
_centered_check y x i.g##i.h, name(factors)
replace x=. in 3
_centered_check y x z if mod(_n,3)!=0 in 7/31001, name(restricted_missing)
replace x=-1 in 3
generate double duplicate=x
_centered_check y x duplicate z, name(duplicate)

* Existing canonical projection owns offset/annihilation boundaries.
generate double offset=1e12+x
quietly fereg y offset z, solver(map)
matrix canonical_b=e(b)
matrix canonical_v=e(V)
local canonical_rank=e(rank)
_centered_marker large_offset_compatibility
quietly fereg y offset z, threads(4)
assert e(rank)==`canonical_rank'
assert mreldif(e(b),canonical_b)==0
assert mreldif(e(V),canonical_v)<1e-12

* The developer output adapter preserves canonical no-FE science for xb alone.
quietly _fereg_estimate_ols_test y x z, solver(map) xb(canonical_xb)
matrix canonical_b=e(b)
matrix canonical_v=e(V)
scalar canonical_rss=e(rss)
foreach workers in 1 4 16 {
    _centered_marker xb_output_`workers'
    quietly _fereg_estimate_ols_test y x z, threads(`workers') xb(actual_xb)
    assert mreldif(e(b),canonical_b)==0 & mreldif(e(V),canonical_v)==0
    assert e(rss)==scalar(canonical_rss)
    assert actual_xb==canonical_xb
    assert "`e(fe_residual_method)'"!="derived"
    drop actual_xb

}
drop canonical_xb

* Selected hidden and public entry points must agree exactly.
quietly fereg y x z, threads(4)
matrix public_b=e(b)
matrix public_v=e(V)
scalar public_rss=e(rss)
quietly _fereg_estimate_ols_test y x z, threads(4)
assert mreldif(e(b),public_b)==0
assert mreldif(e(V),public_v)==0
assert e(rss)==scalar(public_rss)

* Existing error transactions must survive the new path.
quietly regress y x z
matrix before_b=e(b)
local before_cmd `"`e(cmd)'"'
capture noisily _fereg_estimate_ols_test y x z, inject(postplugin) threads(4)
assert _rc==3498
assert e(cmd)==`"`before_cmd'"'
assert mreldif(e(b),before_b)==0
display "END_FIXTURE|ols_no_fe_centered|rc=0"
