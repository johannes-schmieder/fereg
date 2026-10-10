version 18.0
clear all
set more off
set obs 512
generate long row=_n
generate int g=1+mod(floor((_n-1)/8),64)
generate int h=1+mod(g-1+(mod(_n-1,8)==7),64)
generate int j=1+mod(_n-1,3)
generate double x=sin(_n*1.71)
generate double z=cos(_n*0.53)
generate double y=exp(.2*x-.1*z+.2*sin(g)+.1*cos(h))*(.9+.2*mod(_n,5)/4)
generate double w=cond(mod(_n-1,8)==7,.001,1)
// The scarce mobility edges carry little weight. Compare each categorical
// topology with explicit dummy Poisson and preserve scratch-block boundaries.
foreach terms in g "g h" "g h j" {
    local dummy
    foreach term of local terms {
        local dummy `dummy' i.`term'
    }
    quietly poisson y x z `dummy' [pw=w], vce(robust) tolerance(1e-12) ltolerance(1e-12) nrtolerance(1e-12) iterate(200)
    scalar oracle_x=_b[x]
    scalar oracle_z=_b[z]
    predict double oracle_mu, n
    quietly feppml y x z [pw=w], absorb(`terms') memory(fast) solver(map) threads(4) mu(fast_mu)
    matrix fast_b=e(b)
    matrix fast_v=e(V)
    matrix fast_status=e(feppml_status)
    scalar fast_df=e(df_resid)
    scalar fast_rank=e(rank)
    foreach pool in 1 2 10000 {
        quietly feppml y x z [pw=w], absorb(`terms') memory(low) poolsize(`pool') threads(4) mu(low_mu)
        generate double oracle_difference=abs(low_mu-oracle_mu)
        generate double mode_difference=abs(low_mu-fast_mu)
        quietly summarize oracle_difference, meanonly
        display as result "LOW_CATEGORICAL|terms=`terms'|pool=`pool'|oracle_mu_max=" %24.17e r(max) "|x_delta=" %24.17e abs(_b[x]-oracle_x) "|z_delta=" %24.17e abs(_b[z]-oracle_z)
        quietly summarize mode_difference, meanonly
        display as result "LOW_CATEGORICAL_MODES|mu_max=" %24.17e r(max)
        drop oracle_difference mode_difference
        assert e(sample)==1
        assert row==_n
        assert e(num_singletons)==0 & e(separated)==0
        assert abs(_b[x]-oracle_x)<1e-8
        assert abs(_b[z]-oracle_z)<1e-8
        assert abs(low_mu-oracle_mu)<1e-7
        assert abs(low_mu-fast_mu)<1e-7
        assert mreldif(e(b),fast_b)<1e-8
        assert mreldif(e(V),fast_v)<1e-8
        assert mreldif(e(feppml_status),fast_status)==0
        assert e(rank)==fast_rank & e(df_resid)==fast_df
        assert e(feppml_certificate)[16,1]==1
        assert e(feppml_memory_selected_code)==2
        // Scratch may be empty; the admitted estimate is not an allocation peak bound.
        drop low_mu
    }
    drop fast_mu oracle_mu
}
display as result "END_FIXTURE|feppml_low_categorical|rc=0"
