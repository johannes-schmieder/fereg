version 18
clear all
set more off

* Fully factorial positive stencils with different slope coefficients by k.
* The two positive x values force all slopes and all three FE contrasts into
* any strict recession direction. An exact opposite margin blocks deletion
* even when its magnitude falls below the numerical tolerance.
foreach family in coupled near near_blocker near_tiny {
    clear
    local blocks 4
    local cellrows = cond("`family'" == "near", 5, 4)
    quietly set obs `=`blocks'*8*`cellrows''
    generate long id = _n-1
    generate int block = floor(id/(8*`cellrows'))
    generate byte a = mod(floor(id/(4*`cellrows')),2)
    generate byte b = mod(floor(id/(2*`cellrows')),2)
    generate byte c = mod(floor(id/`cellrows'),2)
    generate byte j = mod(id,`cellrows')
    generate int g = 2*block+a+1
    generate int h = 2*block+b+1
    generate int k = 2*block+c+1
    generate double beta = 1+mod(block+c,2)
    generate double center = mod(block,23)/8
    generate double x = center+cond(j==0,-1,cond(j==1,1,0))
    generate byte y = 2*(j<2)
    generate double z = -(2*a+3*b+5*c+x)/beta
    if "`family'" == "coupled" replace z = z-1/beta if j==3
    if "`family'" == "near" {
        replace z = z-2^(-22)/beta if j==3
        replace z = z+2^(-22)/beta if j==4
    }
    if inlist("`family'", "near_blocker", "near_tiny") {
        local power = cond("`family'" == "near_tiny", 40, 14)
        replace z = z-1/beta if j==2
        replace z = z+2^(-`power')/beta if j==3
    }
    generate byte retain = "`family'" != "coupled" | j!=3
    generate int perm = mod(53*id,163)
    sort perm

    quietly count if !retain
    local expected_drops = r(N)
    foreach method in default ir {
        local separation_option = cond("`method'" == "ir", "separation(ir)", "")
        capture quietly feppml y x, absorb(g h k##c.z) threads(1) `separation_option' mu(fit)
        local code = _rc
        if "`family'" == "near_tiny" & inlist(`code',430,459) {
            * A refused certificate or downstream ill-conditioned fit is permissible.
            capture confirm new variable fit
            assert _rc == 0
            continue
        }
        assert `code' == 0
        assert e(converged) == 1
        assert e(num_singletons) == 0
        assert e(separated) == `expected_drops'
        assert e(sample) == retain
        if "`family'" == "coupled" assert abs(fit - 4/3)<1e-7 if retain
        if "`family'" == "near" assert abs(fit - 4/5)<1e-7 if retain
        if inlist("`family'", "near_blocker", "near_tiny") {
            local delta = 2^(-`power')
            local b = -ln(`delta')/(1+`delta')
            local pos = 4/(2+exp(-`b')+exp(`delta'*`b'))
            assert abs(_b[x]-`b')<1e-4
            assert abs(ln(fit) - ln(`pos') - cond(j==2,-`b',cond(j==3,`delta'*`b',0)))<1e-4
        }
        drop fit
    }
}

display "END_FIXTURE|feppml_sparse_support_coupled|rc=0"
