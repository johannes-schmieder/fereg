    * Test-only replacement of the frozen reference's PPML invocation.
    * Wage regressions, reference weights, moment formulas and FEVC stay intact.
    matrix `info'=J(3,6,.)
    local slist
    local tlist
    forvalues z=0/2 {
        local fit "`fits'==1"
        if `z'>0 local fit "`fits'==1 & `branch'==`z'"
        local bcopts "biascorrection(exact)"
        if "`correction'"=="probe" local bcopts "biascorrection(randomized) biasprobes(`probes') biasseed(`seed') biasstream(`z')"
        quietly feppml `sep' `scontrols' if `fit' [pw=`sw'], ///
            absorb(`sepworker' `sepfirm') exposure(`ex') keepsingletons ///
            tolerance(`tolerance') iterate(`maxiter') threads(1) ///
            `bcopts' biasunit(`worker') biasfe(__nb)
        assert e(sample)==(`fit')
        matrix `info'[`z'+1,1]=1
        matrix `info'[`z'+1,2]=0
        matrix `info'[`z'+1,3]=0
        matrix `info'[`z'+1,4]=0
        matrix `info'[`z'+1,5]=e(ic)
        local components=e(feppml_dof)[rownumb(e(feppml_dof),"components"),1]
        quietly levelsof `sepworker' if `fit', local(left_levels)
        quietly levelsof `sepfirm' if `fit', local(right_levels)
        local left_n : word count `left_levels'
        local right_n : word count `right_levels'
        local controls_n : word count `scontrols'
        matrix `info'[`z'+1,6]=`left_n'+`right_n'+`controls_n'-`components'
        quietly summarize `sepfirm' if `reference'==1, meanonly
        local anchor=r(min)
        foreach channel in raw bc {
            quietly summarize __nb_2_`channel' if `sepfirm'==`anchor' & `fit', meanonly
            if r(N)==0 error 459
            local shift=r(mean)
            quietly replace __nb_1_`channel'=__nb_1_`channel'+`shift'
            quietly replace __nb_2_`channel'=__nb_2_`channel'-`shift'
            tempvar s t
            quietly bysort `sepworker': egen double `s'=min(__nb_1_`channel')
            quietly bysort `sepfirm': egen double `t'=min(__nb_2_`channel')
            assert !missing(`s',`t') if `reference'==1
            local slist `slist' `s'
            local tlist `tlist' `t'
        }
        drop __nb_*
    }
    mata: ev_native__moments("`branch'","`reference'","`refweight'", ///
        "`alist'","`plist'","`slist'","`tlist'","`out'")
