version 17.0
* Science-only coverage of the newly eligible wide loading families.
args science threads trace
quietly generate int h = mod(_n,97)
quietly generate double w = 1 + mod(_n,5)
quietly generate double off = .01*mod(_n,7)
quietly generate double expo = exp(off)
quietly generate double py = exp(.2*x1 + .1*x2 + sin(g/11)/4)*expo
local opts1 "absorb(g) aweight(w)"
local opts2 "absorb(g) fweight(w)"
local opts3 "absorb(g) pweight(w)"
local opts4 "absorb(g h) vce(cluster) cluster(h)"
local opts5 "absorb(g##c.x1)"
local opts6 "absorb(g) memory(low)"
local opts7 "absorb(g)"
local opts8 "absorb(g) exposure(expo)"
local opts9 "absorb(g##c.x1) offset(off)"
tempname B1 V1 B V S
forvalues case = 1/9 {
    preserve
    if `case' == 7 quietly replace x8 = .a in 4097
    local outcome = cond(`case' >= 8, "py", "y")
    local command = cond(`case' >= 8, "_fereg_estimate_ppml", "_fereg_estimate_ols_test")
    local opts `opts`case''
    file open widetrace using `"`trace'"', write append text
    file write widetrace "FEREG_SOLVER_TRACE|phase=width_extra|case=`case'|mode=reference|threads=1" _n
    file close widetrace
    quietly `command' `outcome' x1-x64, `opts' threads(1)
    matrix `B1' = e(b)
    matrix `V1' = e(V)
    tempvar sample1
    quietly generate byte `sample1' = e(sample)
    foreach t of local threads {
        foreach mode in serial parallel repeat {
            local inject = cond("`mode'"=="serial", "inject(serial_input)", "")
            file open widetrace using `"`trace'"', write append text
            file write widetrace "FEREG_SOLVER_TRACE|phase=width_extra|case=`case'|mode=`mode'|threads=`t'" _n
            file close widetrace
            quietly `command' `outcome' x1-x64, `opts' threads(`t') `inject'
            assert mreldif(e(b),`B1') < 1e-9
            assert mreldif(e(V),`V1') < 1e-8
            assert e(sample) == `sample1'
            if "`mode'" == "serial" {
                matrix `B' = e(b)
                matrix `V' = e(V)
                scalar `S' = e(N)
            }
            else {
                assert mreldif(e(b),`B') == 0
                assert mreldif(e(V),`V') == 0
                assert e(N) == scalar(`S')
            }
            quietly _datasignature `sample1', nonames
            local sample_signature `"`r(datasignature)'"'
            mata: fh=fopen(st_local("science"),"a"); fput(fh,"extra"+st_local("case")+"|"+invtokens(strofreal(vec(st_matrix("e(b)"))',"%21x"))+"|"+invtokens(strofreal(vec(st_matrix("e(V)"))',"%21x"))+"|"+strofreal(st_numscalar("e(N)"),"%21x")+"|"+st_local("sample_signature")); fclose(fh)
        }
    }
    restore
}
