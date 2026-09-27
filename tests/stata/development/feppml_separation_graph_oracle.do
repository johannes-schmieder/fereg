version 18.0
clear all
set more off
set obs 1000
generate byte bridge = _n > 960
generate byte component = mod(_n,2)
generate int g = 1 + mod(floor((_n-1)/2),4) + 4*component
generate int h = 1 + mod(floor((_n-1)/7),3) + 3*component
replace g = 1 + mod(_n,4) if bridge
replace h = 4 + mod(_n,3) if bridge
generate int j = 1 + mod(_n,5)
generate int cl = 1 + mod(_n,19)
generate double x1 = sin(_n/13) + mod(_n,7)/5
generate double x2 = cos(_n/17) - mod(_n,11)/9
generate double off = .1*sin(_n/31)
generate double y = 1 + mod(floor(100*exp(.2*x1-.1*x2+.03*g+off)),19)
replace y = 0 if bridge
generate int fw = 1 + mod(_n,3)
generate double pw = .2 + mod(_n,13)/3
generate double raw_w = 1
* Independent exact direction: alpha(g<=4)=0, alpha(g>4)=-1;
* beta(h<=3)=0, beta(h>3)=1. Its negative is a recession direction.
generate double exact_direction = (g>4) - (h>3)
assert exact_direction == 0 if !bridge
assert exact_direction == -1 if bridge

mata:
void graph_sandwich(string scalar kind, string scalar covariance, real scalar dimensions)
{
    selected = st_data(., "native_sample")
    data = select(st_data(., ("x1","x2","g","h","j","cl","y","mu_dummy","raw_w")), selected)
    X = data[.,1..2]
    for (l=1;l<=8;l++) X = (X, data[.,3]:==l)
    for (l=1;l<=6;l++) X = (X, data[.,4]:==l)
    if (dimensions==3) for (l=1;l<=5;l++) X = (X, data[.,5]:==l)
    y = data[.,7]; mu = data[.,8]; w = data[.,9]
    bread = invsym(quadcross(X,w:*mu,X))
    if (covariance=="cluster") {
        scores = J(19,cols(X),0)
        for (i=1;i<=rows(X);i++) scores[data[i,6],.] = scores[data[i,6],.] + w[i]*(y[i]-mu[i])*X[i,.]
        meat = quadcross(scores,scores)
    }
    else {
        score_w = w:^2
        if (kind=="fw") score_w=w
        meat = quadcross(X,score_w:*(y-mu):^2,X)
    }
    V = bread*meat*bread*st_numscalar("correction")
    st_matrix("V_oracle",V[1..2,1..2])
}
end

foreach dimensions in 2 3 {
    local effects "g h"
    local dummies "ibn.g i.h"
    if `dimensions' == 3 {
        local effects "g h j"
        local dummies "ibn.g i.h i.j"
    }
    foreach kind in unit fw pw {
        local weight ""
        replace raw_w = 1
        if "`kind'" != "unit" {
            local weight "[`kind'=`kind']"
            replace raw_w = `kind'
        }
        foreach mode in fast low {
            local threads = cond("`mode'"=="fast",4,1)
            local covariance = cond("`kind'"=="pw","cluster","robust")
            local vce "robust"
            if "`covariance'" == "cluster" local vce "cluster cl"
            * IR-only forces the new starting proposal through full-model span
            * validation and a fresh terminal-generation IR replay.
            feppml y x1 x2 `weight', absorb(`effects') offset(off) vce(`vce') ///
                separation(ir) keepsingletons memory(`mode') threads(`threads') ///
                mu(mu_native) eta(eta_native) residuals(res_native)
            assert e(converged)==1
            generate byte native_sample = e(sample)
            assert native_sample == !bridge
            assert missing(mu_native) if bridge
            assert abs(res_native-y+mu_native) < 1e-9 if native_sample
            matrix b_native=e(b)
            matrix V_native=e(V)
            scalar correction=e(feppml_inference)[3,1]
            quietly poisson y x1 x2 `dummies' if !bridge `weight', noconstant offset(off) nolog
            assert abs(_b[x1]-b_native[1,1])<1e-7
            assert abs(_b[x2]-b_native[1,2])<1e-7
            predict double mu_dummy if !bridge,n
            assert abs(mu_native-mu_dummy)<1e-7*(1+abs(mu_dummy)) if !bridge
            mata: graph_sandwich("`kind'","`covariance'",`dimensions')
            matrix V_slopes=V_native[1..2,1..2]
            assert mreldif(V_slopes,V_oracle)<1e-7
            drop native_sample mu_native eta_native res_native mu_dummy
        }
    }
}
display as result "END_FIXTURE|feppml_separation_graph_oracle|rc=0"
