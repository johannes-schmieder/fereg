version 18.0
clear all
set more off
set obs 384
generate byte group = mod(_n - 1, 6)
generate long period = floor((_n - 1) / 6)
generate byte clusterid = mod(period + group, 19)
generate double x = sin(.31 * period) + cos(.21 * period + group)
generate double z = .4 * x + mod(3 * period + group, 17) / 17
generate double y = .7 * x - .2 * z + group + sin(.73 * period + group)
generate double x_scaled = x * 1e4
generate double z_scaled = z / 1e4
matrix D = (1e4,0,0 \ 0,1e-4,0 \ 0,0,1)
foreach variance in robust "cluster clusterid" {
    quietly areg y x z, absorb(group) vce(`variance')
    matrix b_ref = e(b)
    matrix V_ref = e(V)
    foreach mode in fast low {
        quietly fereg y x_scaled z_scaled, absorb(group) vce(`variance') memory(`mode') threads(1)
        matrix b_common = e(b) * D
        matrix V_common = D * e(V) * D
        assert mreldif(b_common, b_ref) < 1e-10
        assert mreldif(V_common, V_ref) < 1e-10
    }
}
display "END_FIXTURE|ols_score_design_units|rc=0"
