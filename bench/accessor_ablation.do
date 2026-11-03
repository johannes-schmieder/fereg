version 17.0
* Same-data, same-binary OLS reader ablation. See accessor_ablation.py.
args mode output rows controls threads pairs seed
set more off
set processors 8
if "`mode'" == "verify" {
    clear
    set seed `seed'
    quietly set obs `rows'
    forvalues j = 1/`controls' {
        quietly generate double x`j' = rnormal()
    }
    quietly generate long g = mod(_n*31,5003)+1
    quietly generate double y = x1+.5*x2-.25*x3+.1*x4+sin(g/11)+rnormal()
    quietly save `"`output'/input.dta"'
    quietly areg y x1-x`controls', absorb(g)
    matrix B0 = e(b)
    matrix V0 = e(V)
    assert e(N) == `rows'
    assert e(sample) == 1
    tempname sci
    file open `sci' using `"`output'/science.csv"', write text
    file write `sci' "fit,max_abs_b_vs_areg,max_abs_V_vs_areg,n,df_r,df_a,rank" _n
    local trace : environment FEREG_SOLVER_TRACE_FILE
    foreach arm in serial1 serial2 parallel1 parallel2 public one {
        tempname tr
        file open `tr' using `"`trace'"', write append text
        file write `tr' "FEREG_SOLVER_TRACE|phase=accessor_ablation|arm=`arm'" _n
        file close `tr'
        local command = cond(inlist("`arm'","public","one"),"fereg","_fereg_estimate_ols_test")
        local injection = cond(substr("`arm'",1,6)=="serial","inject(serial_input)","")
        local workers = cond("`arm'"=="one",1,`threads')
        quietly `command' y x1-x`controls', absorb(g) threads(`workers') `injection'
        matrix B = e(b)
        matrix V = e(V)
        mata: st_numscalar("bdiff", max(abs(st_matrix("B")-st_matrix("B0"))))
        mata: st_numscalar("vdiff", max(abs(st_matrix("V")-st_matrix("V0"))))
        assert bdiff < 1e-8 & vdiff < 1e-8
        assert e(N) == `rows'
        assert e(sample) == 1
        matrix D = (e(N),e(df_r),e(df_a),e(rank))
        if "`arm'" == "serial1" {
            matrix B1 = B
            matrix V1 = V
            matrix D1 = D
            quietly estimates save `"`output'/baseline.ster"'
        }
        else if "`arm'" == "one" {
            mata: assert(max(abs(st_matrix("B")-st_matrix("B1"))) < 1e-9)
            mata: assert(max(abs(st_matrix("V")-st_matrix("V1"))) < 1e-8)
        }
        else {
            assert mreldif(B,B1) == 0
            assert mreldif(V,V1) == 0
        }
        assert mreldif(D,D1) == 0
        file write `sci' "`arm',`=scalar(bdiff)',`=scalar(vdiff)',`e(N)',`e(df_r)',`e(df_a)',`e(rank)'" _n
    }
    file close `sci'
}
else if "`mode'" == "timing" {
    quietly use `"`output'/input.dta"', clear
    quietly estimates use `"`output'/baseline.ster"'
    matrix B1 = e(b)
    matrix V1 = e(V)
    matrix D1 = (e(N),e(df_r),e(df_a),e(rank))
    * One warmup per reader; no trace in any timed call.
    quietly _fereg_estimate_ols_test y x1-x`controls', absorb(g) threads(`threads') inject(serial_input)
    quietly _fereg_estimate_ols_test y x1-x`controls', absorb(g) threads(`threads')
    tempname handle
    file open `handle' using `"`output'/results.csv"', write append text
    forvalues pair = 1/`pairs' {
        local order "incumbent candidate"
        if mod(`pair',2)==0 local order "candidate incumbent"
        foreach arm of local order {
            local injection = cond("`arm'"=="incumbent","inject(serial_input)","")
            timer clear 1
            timer on 1
            quietly _fereg_estimate_ols_test y x1-x`controls', absorb(g) threads(`threads') `injection'
            timer off 1
            quietly timer list 1
            local wall = r(t1)
            local load = el(e(fereg_runtime),19,2)
            local native = el(e(fereg_runtime),21,2)
            local alloc = el(e(fereg_runtime),12,2)
            file write `handle' "`arm',`pair',loading,`threads',1,`rows',`e(N)',`wall',`load',`native',`alloc'" _n
            matrix B = e(b)
            matrix V = e(V)
            matrix D = (e(N),e(df_r),e(df_a),e(rank))
            assert mreldif(B,B1) == 0 & mreldif(V,V1) == 0 & mreldif(D,D1) == 0
            assert e(sample) == 1
        }
    }
    file close `handle'
}
else exit 198
display as result "END_FIXTURE|accessor_ablation|rc=0"
