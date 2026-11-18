version 18
clear all
set more off

* A cycle of two-way FE cells, each containing three linearly independent
* positive row types. Only t can separate in the mixed case. In the opposed
* case, two opposite constraints on z keep both disputed zeros. The third
* case admits a single direction with different strict margins.
foreach shape in mixed opposed continuous {
    clear
    quietly set obs 224
    generate int id = _n-1
    generate int edge = floor(id/8)
    generate int g = mod(floor(edge/2),14)
    generate int h = mod(g+mod(edge,2),14)
    generate byte j = mod(id,8)
    generate double y = cond(j<6,1+mod(floor(j/3),2),0)
    generate double x = cond(j<6,mod(j,3),0)
    generate double z = cond(j<6,x+2^(-30)*(x==2),j==6)
    generate double t = j==7
    if "`shape'"=="opposed" {
        replace z = cond(j<6,0,cond(j==6,-1,1))
        replace y = 0 if j==5
        replace t = j==5
    }
    if "`shape'"=="continuous" {
        replace z = cond(j<6,0,cond(j==6,-.25,-sqrt(2)))
        replace t = 0
    }
    replace z = (z-x)*2^21
    replace x = -3*x
    generate int perm = mod(53*id,227)
    sort perm
    generate byte retain = cond("`shape'"=="continuous",j<6, ///
        cond("`shape'"=="opposed",j!=5,j!=7))
    foreach method in "ir" "fe simplex" "fe simplex ir" {
        quietly feppml y x z t, absorb(g h) separation(`method') threads(1)
        assert e(sample)==retain
    }
}

display "END_FIXTURE|feppml_exact_primal_support|rc=0"
