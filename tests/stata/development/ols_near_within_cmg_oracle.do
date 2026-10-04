version 18.0
clear all
set more off
set obs 48
generate byte g = 1 + mod(_n-1,4)
generate byte h = 1 + mod(floor((_n-1)/4),3)
generate byte replicate = floor((_n-1)/12)
generate double u = cond(replicate<2,1,-1)
generate double v = cond(mod(replicate,2)==0,1,-1)
generate double noise = 4*u*v
generate double x1 = u + 2e-9*g
generate double x2 = v + 1e-9*h
generate double y = .7*x1 - .4*x2 + 2e-9*g + 1e-9*h + noise
generate double aw = .25 + g/4 + h/8
generate double pw = aw
generate int fw = 1 + g + h

* Each FE tuple has four equally weighted Hadamard replicates. The noise is
* orthogonal to both controls and every explicit FE dummy. Thus beta=(.7,-.4)
* and residual=noise independently of either package's projection recurrence.
* Explicit dummies additionally check the weighted finite solve and HC1 VCE.
foreach kind in unit aw fw pw {
    local weight ""
    if "`kind'" != "unit" local weight "[`kind'=`kind']"
    quietly regress y x1 x2 i.g i.h `weight', vce(robust)
    matrix oracle_b = e(b)[1,1..2]
    matrix oracle_V = e(V)[1..2,1..2]
    scalar oracle_N = e(N)
    scalar oracle_df = e(df_r)
    predict double oracle_residual, residuals
    assert abs(oracle_b[1,1]-.7)<1e-10
    assert abs(oracle_b[1,2]+.4)<1e-10
    assert abs(oracle_residual-noise)<1e-9
    foreach mode in fast low {
        foreach t in 1 4 {
            foreach solver in auto cmg {
                quietly fereg y x1 x2 `weight', absorb(g h) solver(`solver') ///
                    memory(`mode') poolsize(1) threads(`t') tolerance(1e-12) ///
                    vce(robust) residuals(native_residual)
                assert e(converged)==1
                assert e(fereg_certificate)[32,2]==1
                assert e(sample)==1
                assert e(N)==oracle_N
                assert e(df_r)==oracle_df
                if "`solver'"=="cmg" {
                    assert e(fereg_solver)[1,1]==7
                    assert inlist(e(fereg_solver)[1,2],4,7)
                    if e(fereg_solver)[1,2]==4 {
                        assert e(fereg_solver)[1,11]==1
                        assert e(fereg_solver)[1,12]>0
                    }
                }
                matrix finite_b = e(b)[1,1..2]
                matrix finite_V = e(V)[1..2,1..2]
                assert mreldif(finite_b,oracle_b)<1e-10
                assert mreldif(finite_V,oracle_V)<1e-10
                assert abs(native_residual-noise)<1e-9
                drop native_residual
            }
        }
    }
    * Recovery must retain a nonzero FE contribution at its original scale.
    foreach mode in fast low {
        foreach t in 1 4 {
            quietly fereg y x1 x2 `weight', absorb(left=g right=h) solver(cmg) ///
                memory(`mode') poolsize(1) threads(`t') tolerance(1e-12) ///
                vce(robust) residuals(saved_residual)
            matrix finite_b = e(b)[1,1..2]
            matrix finite_V = e(V)[1..2,1..2]
            assert mreldif(finite_b,oracle_b)<1e-10
            assert mreldif(finite_V,oracle_V)<1e-10
            assert e(fereg_certificate)[32,2]==1
            predict double fitted, xbd
            predict double xb, xb
            assert abs(saved_residual-noise)<1e-9
            assert abs(y-fitted-saved_residual)<1e-14
            assert abs(fitted-xb-left-right)<1e-14
            assert abs(y-xb-noise-left-right)<1e-14
            drop left right saved_residual fitted xb
        }
    }
    drop oracle_residual
}
display as result "END_FIXTURE|ols_near_within_cmg_oracle|rc=0"
