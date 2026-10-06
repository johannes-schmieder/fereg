version 18.0
clear all
set more off
set obs 120
generate int g = ceil(_n/10)
generate byte h = mod(_n,2)+1
generate byte f = 1
generate byte c = mod(_n,7)+1
generate double x = sin(_n*.31)+cos(_n*.17)
clonevar duplicate = x
generate double y = 1+.7*x+.3*h+.2*g+.05*sin(_n*.73)
generate double aw = .5+mod(_n,11)/7
generate int fw = 1+mod(_n,3)
generate double pw = .7+mod(_n,13)/9
tempfile data
save `data'
// The duplicate and constant factor are absent from the accepted finite basis.
// Built-in explicit-dummy WLS independently witnesses the identified slope
// covariance and residuals across every supported weighted sandwich family.
foreach weight in aw fw pw {
    foreach vce in robust cluster {
        foreach memory in fast low {
            foreach workers in 1 2 4 8 16 {
                use `data', clear
                local inference vce(robust)
                if "`vce'" == "cluster" local inference vce(cluster c)
                quietly fereg y x duplicate i.h i.f [`weight'=`weight'], ///
                    absorb(saved=g) `inference' memory(`memory') ///
                    threads(`workers') residuals(ours)
                matrix b = e(b)
                matrix V = e(V)
                scalar dfr = e(df_r)
                generate byte sample = e(sample)
                local names : colfullnames b
                assert strpos("`names'", "o.duplicate") > 0
                scalar beta_x = _b[x]
                scalar beta_h = _b[2.h]
                local ix = colnumb(b,"x")
                local ih = colnumb(b,"2.h")
                quietly regress y x i.h i.g [`weight'=`weight'] if sample, `inference'
                matrix reference = e(V)
                assert abs(beta_x-_b[x]) < 1e-8
                assert abs(beta_h-_b[2.h]) < 1e-8
                assert dfr == e(df_r)
                local rx = colnumb(e(b),"x")
                local rh = colnumb(e(b),"2.h")
                assert abs(V[`ix',`ix']-reference[`rx',`rx']) < 1e-8
                assert abs(V[`ih',`ih']-reference[`rh',`rh']) < 1e-8
                assert abs(V[`ix',`ih']-reference[`rx',`rh']) < 1e-8
                predict double reference_residual, residuals
                assert abs(ours-reference_residual) < 1e-8 if sample
            }
        }
    }
}
display "END_FIXTURE|rank_pruned_products|rc=0"
