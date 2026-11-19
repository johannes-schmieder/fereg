version 18.0
clear all
set more off
set obs 50000
generate byte group = mod(floor((_n - 1) / 12), 3)
generate byte category = cond(_n == 1, 0, mod(_n, 12) + 1)
generate byte clusterid = mod(floor(_n / 12), 32)
generate double y = sin(_n * .31) + mod(_n, 5) / 7 + category / 3
forvalues j = 1/12 {
    generate byte d`j' = category == `j'
}
* Give areg an abundant reference category. The original rare base makes its
* normalized equations ill-conditioned. Convert the reference coefficients
* and covariance back to the original basis before comparing.
generate byte rare = category == 0
matrix C = I(13)
forvalues j = 1/12 {
    matrix C[`j',12] = -1
}
matrix C[13,12] = 1
foreach variance in robust "cluster clusterid" {
    quietly areg y d1-d11 rare, absorb(group) vce(`variance')
    matrix b_ref = e(b) * C'
    matrix V_ref = C * e(V) * C'
    foreach mode in fast low {
        quietly fereg y d1-d12, absorb(group) vce(`variance') memory(`mode') threads(1)
        assert e(N) == 50000
        assert mreldif(e(b), b_ref) < 1e-8
        assert mreldif(e(V), V_ref) < 1e-8
    }
}
display "END_FIXTURE|ols_score_design_rare_base|rc=0"
