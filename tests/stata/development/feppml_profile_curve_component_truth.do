version 18.0
clear all
set more off
set obs 480
generate long pair = floor((_n - 1) / 2)
generate byte component = floor(pair / 120)
generate int g = 1 + mod(pair, 5) + 5*component
generate int h = 1 + mod(floor(pair / 5), 4) + 4*component
generate double x = sin(pair/17) + mod(pair,7)/5
generate double off = 0.1*cos(pair/19)
generate double truth = .3 + .2*x + .1*g - .07*h + off
generate double y = exp(truth) * cond(mod(_n,2),.9,1.1)
generate double q = 1
foreach disparity in 1 1e13 {
    replace q = (1+mod(pair,11)/7) * cond(component,`disparity',1)
    foreach mem in low fast {
        foreach t in 1 4 {
            feppml y x [pw=q], absorb(g h) offset(off) separation(none) keepsingletons ///
                vce(robust) memory(`mem') threads(`t') mu(fitted) eta(predictor)
            assert e(converged) == 1
            assert abs(_b[x] - .2) < 1e-10
            assert abs(predictor-truth) < 1e-10*(1+abs(truth))
            assert abs(fitted-exp(truth)) < 1e-10*(1+exp(truth))
            assert e(sample) == 1
            drop fitted predictor
        }
    }
}
display as result "END_FIXTURE|feppml_profile_curve_component_truth|rc=0"
