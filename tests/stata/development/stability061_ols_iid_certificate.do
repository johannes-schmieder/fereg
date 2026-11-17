version 18.0
clear all
set more off

* Representative generated IID-certificate witness, not the unavailable
* data/options from the user's report. The sparse graph has two connected
* half-blocks joined by edge(7,8), 129 edges and 4 complete Walsh rows each.
* The table records the exact wrapping-u64 LCG choices for seed(g+7)*7919,
* multiplier6364136223846793005, increment1; retain these public inputs.
input byte(g h1 h2 h3 h4 h5 h6 h7 h8)
0 0 1 6 0 6 6 6 5
1 1 7 2 7 3 7 7 2
2 2 5 7 6 1 0 7 7
3 3 4 3 5 6 2 7 4
4 4 2 7 5 3 3 0 1
5 5 0 4 4 1 5 0 6
6 6 7 0 3 6 6 0 3
7 7 5 5 2 3 7 1 1
8 8 11 9 9 8 9 9 14
9 9 10 14 8 14 10 9 11
10 10 8 10 8 11 12 10 8
11 11 14 14 15 8 13 10 13
12 12 12 11 14 13 14 10 10
13 13 11 15 13 11 8 11 8
14 14 9 12 12 8 9 11 13
15 15 15 8 11 13 11 12 10
end
reshape long h, i(g) j(within_edge)
sort g within_edge
drop within_edge
set obs 129
replace g=7 in 129
replace h=8 in 129
generate int edge=_n
expand 4
bysort edge: generate byte r=_n-1
generate double s1=1-2*mod(r,2)
generate double s3=1-2*mod(mod(r,2)+floor(r/2),2)
generate double fa=cos(c(pi)*(g+.5)/16)+.2*sin(1.713*g)
generate double fb=cos(c(pi)*(h+.5)/16)+.2*sin(.713*h)
generate double signal=1e6*(fa+fb)
generate double x=s1+1000*(-fa-2*fb)
* Match the source witness's arithmetic order; all four rows share its FE part.
generate double y=(signal+.2*s3)+.01*s1
generate double w=1+mod(edge-1,3)
assert _N==516

* Each edge contains balanced orthogonal s1/s3 channels. Edge-constant AW/FW
* preserve zero weighted FE moments and make beta, RSS, within TSS and F known
* without an iterative estimator or a dense dummy inversion.
foreach kind in unit aw fw {
    local weight
    generate double reference_weight=1
    if "`kind'"!="unit" {
        local weight "[`kind'=w]"
        replace reference_weight=w
    }
    if "`kind'"=="aw" {
        quietly summarize w, meanonly
        replace reference_weight=w/r(mean)
    }
    quietly summarize reference_weight, meanonly
    scalar reference_n=r(sum)
    scalar reference_df=reference_n-32
    scalar reference_rss=.04*reference_n
    scalar reference_within=.0401*reference_n
    scalar reference_F=.0025*reference_df
    scalar reference_V=.04/reference_df
    quietly summarize signal [aw=reference_weight], meanonly
    generate double centered_signal_sq=reference_weight*(signal-r(mean))^2
    quietly summarize centered_signal_sq, meanonly
    scalar reference_tss=r(sum)+reference_within

    foreach policy in lsmr tight auto {
        local options
        if "`policy'"=="lsmr" local options solver(lsmr)
        if "`policy'"=="tight" local options tolerance(1e-12)
        foreach output in none residual {
            local output_option
            if "`output'"=="residual" local output_option residuals(actual_r)
            * SPEC permits tolerance-level (~1e-5) derived-statistic differences
            * on weak graphs. Output recovery or a strict solve must meet the
            * stronger oracle; never compare rounded large-origin RSS as exact.
            local statistic_tolerance=1e-7
            if "`policy'"=="auto" & "`output'"=="none" local statistic_tolerance=1e-5
            foreach workers in 1 8 16 {
                display "IID_WALSH_CASE|`kind'|`policy'|`output'|workers=`workers'"
                quietly fereg y x `weight', absorb(g h) threads(`workers') ///
                    `options' `output_option'
                assert e(sample)==1
                assert e(N)==reference_n
                assert e(num_singletons)==0
                assert e(df_a)==31 & e(df_m)==1 & e(df_r)==reference_df
                local names : colnames e(b)
                assert "`names'"=="x _cons"
                assert abs(_b[x]-.01)<1e-8
                assert abs(e(rss)-reference_rss)<`statistic_tolerance'*reference_rss
                assert abs(e(tss_within)-reference_within)<`statistic_tolerance'*reference_within
                assert abs(e(tss)-reference_tss)<1e-12*reference_tss
                assert abs(e(F)-reference_F)<`statistic_tolerance'*reference_F
                assert abs(e(V)[1,1]-reference_V)<`statistic_tolerance'*reference_V
                assert abs(e(r2_within)-(1-.04/.0401))<`statistic_tolerance'*(1-.04/.0401)
                if "`output'"=="residual" {
                    assert abs(actual_r-.2*s3)<1e-7
                    drop actual_r
                }
            }
        }
    }
    drop reference_weight centered_signal_sq
}

display as result "END_FIXTURE|stability061_ols_iid_certificate|rc=0"
