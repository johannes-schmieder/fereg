version 18.0
clear all
set more off
set processors 4

// Full supported thread/output/VCE cube across the six M26 model strata.
// Captures are compared to the accepted native parent; existing dense,
// original-axis, strict PPML and saved-slope witnesses supply independent science.
mata:
void product_capture(string scalar label, real matrix values) {
    real scalar i, j
    for (i=1; i<=rows(values); i++) for (j=1; j<=cols(values); j++)
        printf("EXACT|%s|%g|%g|%21x\n", label, i, j, values[i,j])
}
end
set obs 288
generate long id = _n
generate byte g = 1+mod(_n-1,6)
generate byte h = 1+mod(floor((_n-1)/6),8)
generate byte k = 1+mod(floor((_n-1)/48),6)
generate byte f = 1+mod(_n,3)
generate byte c1 = 1+mod(_n,12)
generate byte c2 = 1+mod(floor((_n-1)/4),9)
generate double z = .5+mod(7*_n,23)/19
generate double x = sin(_n*.31)+cos(_n*.17)
generate double x2 = cos(_n*.43)+sin(_n*.11)
generate double linear = 1+.7*x-.2*x2+.03*g-.02*h+.04*k+.05*f
// Positive, nonseparated outcome; avoid an exactly fitted covariance.
generate double yp = exp(.1*x-.05*x2+.015*g-.01*h+.005*k+.01*f) * (1+.07*sin(_n*.73))
generate double yo = linear+.05*sin(_n*.73)
tempfile data
save `data'
local cells 0
foreach model in ols ppml {
    local command fereg
    local y yo
    local vces iid robust cluster multiway
    if "`model'" == "ppml" {
        local command feppml
        local y yp
        local vces robust cluster multiway
    }
    foreach family in finite one two three factor slope {
        local rhs x x2
        if "`family'" == "factor" local rhs x x2 i.f
        local effects
        if "`family'" == "one" local effects g
        if inlist("`family'", "two", "factor") local effects g h
        if "`family'" == "three" local effects g h k
        if "`family'" == "slope" local effects g##c.z
        local outputs none residual fe combined
        if "`family'" == "finite" local outputs none residual
        foreach vce of local vces {
            local inference
            if "`vce'" == "robust" local inference vce(robust)
            if "`vce'" == "cluster" local inference vce(cluster c1)
            if "`vce'" == "multiway" local inference vce(cluster c1 c2)
            foreach memory in fast low {
                foreach workers in 1 2 4 8 16 {
                    foreach output of local outputs {
                        use `data', clear
                        local absorption
                        local requested `effects'
                        if inlist("`output'", "fe", "combined") {
                            local requested savedg=g
                            if inlist("`family'", "two", "three", "factor") local requested `requested' savedh=h
                            if "`family'" == "three" local requested `requested' savedk=k
                            if "`family'" == "slope" local requested savedg=g##c.z
                        }
                        if "`effects'" != "" local absorption absorb(`requested')
                        local extra
                        if "`model'" == "ppml" local extra separation(none)
                        if inlist("`output'", "residual", "combined") {
                            local extra `extra' residuals(residual)
                            if "`model'" == "ppml" local extra `extra' xb(xb) eta(total) mu(mean) d(fe)
                        }
                        quietly `command' `y' `rhs', `absorption' `inference' ///
                            memory(`memory') threads(`workers') `extra'
                        assert e(N) == 288 & e(sample) == 1
                        matrix b = e(b)
                        matrix V = e(V)
                        local names : colfullnames b
                        local label `model'-`family'-`vce'-`memory'-`workers'-`output'
                        display "EXACT|NAMES|`label'|`names'"
                        mata: product_capture("`label'-B", st_matrix("b"))
                        mata: product_capture("`label'-V", st_matrix("V"))
                        foreach scalar in N df_m df_a df_r rank {
                            capture noisily display "EXACT|S|`label'-`scalar'|" %21x e(`scalar')
                        }
                        if inlist("`output'", "residual", "combined") {
                            if "`model'" == "ols" {
                                quietly predict double xb, xb
                                quietly predict double total, xbd
                                quietly predict double fe, d
                            }
                            if "`model'" == "ols" assert abs(`y'-total-residual) < 1e-8
                            else assert abs(`y'-mean-residual) < 1e-7 & abs(mean-exp(total)) < 1e-7
                            assert abs(total-xb-fe) < 1e-7
                            mata: product_capture("`label'-outputs", st_data(., ("residual","xb","total","fe")))
                        }
                        if inlist("`output'", "fe", "combined") {
                            local channels savedg
                            if "`family'" == "slope" local channels savedg savedg_Slope1
                            if inlist("`family'", "two", "three", "factor") local channels savedg savedh
                            if "`family'" == "three" local channels savedg savedh savedk
                            mata: product_capture("`label'-channels", st_data(., tokens("`channels'")))
                        }
                        local ++cells
                    }
                }
            }
        }
    }
}
assert `cells' == 1540
display "PRODUCT_CUBE|cells=`cells'|workers=1,2,4,8,16|memory=fast,low"
display "END_FIXTURE|accepted_product_interactions|rc=0"
