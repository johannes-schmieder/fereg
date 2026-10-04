version 18.0
clear all
set more off
set obs 120
generate int g = ceil(_n/10)
generate byte f = 1
generate byte h = mod(_n,2)+1
generate byte q = mod(_n,3)+1
generate double x = sin(_n*.31)+cos(_n*.17)
generate double y = 1+.7*x+.3*h+.2*g+.05*sin(_n*.73)
tempfile source
save `source'

// Built-in dummy OLS independently identifies the slope, sandwich and fitted
// values despite redundant constant factors and different finite QR bases.
foreach rhs in i.f i.f#i.h i.f##i.h i.h#i.f i.h##i.f 2.q {
    foreach mode in ordinary saved {
        foreach memory in fast low {
            foreach workers in 1 4 {
                use `source', clear
                local effect g
                if "`mode'" == "saved" local effect channel=g
                quietly fereg y x `rhs', absorb(`effect') vce(robust) ///
                    memory(`memory') threads(`workers') residuals(ours)
                matrix b = e(b)
                matrix V = e(V)
                scalar dfr = e(df_r)
                generate byte sample = e(sample)
                local names : colfullnames b
                assert strpos("`names'", "1b.f") == 0
                quietly regress y x `rhs' i.g if sample, vce(robust)
                assert abs(b[1,1]-_b[x]) < 1e-8
                assert abs(V[1,1]-e(V)[1,1]) < 1e-8
                assert dfr == e(df_r)
                predict double reference_residual, residuals
                assert abs(ours-reference_residual) < 1e-8 if sample
            }
        }
    }
}

// Independent Poisson cell means and HC1 covariance on identified contrasts.
clear
input double(y x) byte h
.5 0 1
1 0 1
1.5 0 1
1 1 2
3 1 2
5 1 2
end
generate byte f = 1
tempfile cells
save `cells'
foreach scale in 1 2 {
    foreach memory in fast low {
        foreach workers in 1 4 {
            foreach rhs in "x" "x i.f" "x f" {
                use `cells', clear
                replace y = `scale'*y
                quietly feppml y `rhs', separation(none) memory(`memory') threads(`workers')
                matrix b = e(b)
                matrix V = e(V)
                assert abs(b[1,1]-ln(3)) < 1e-7
                assert abs(V[1,1]-((6/5)*(.5/9+8/81))) < 1e-9
                assert e(df_chi2) == 1 & e(df_m) == 1
                assert abs(e(chi2)-(27/5)*ln(3)^2) < 1e-7
                if "`rhs'" != "x" {
                    assert colsof(b) == 3
                    assert abs(b[1,2]+b[1,3]-ln(`scale')) < 1e-7
                    assert abs(V[2,2]+V[3,3]+2*V[2,3]-(6/5)*.5/9) < 1e-9
                    assert abs(V[1,2]+V[1,3]+(6/5)*.5/9) < 1e-9
                }
                predict double mean, mu
                assert abs(mean-`scale'*cond(x==0,1,3)) < 1e-7
            }
        }
    }
}
foreach rhs in i.f#i.h i.f##i.h i.h#i.f i.h##i.f {
    use `cells', clear
    quietly feppml y `rhs', separation(none) threads(1)
    matrix b = e(b)
    matrix V = e(V)
    if strpos("`rhs'", "##") == 0 matrix A = (1,0,1 \ 0,1,1)
    if "`rhs'" == "i.f##i.h" matrix A = (1,0,0,0,0,1 \ 1,0,1,0,1,1)
    if "`rhs'" == "i.h##i.f" matrix A = (0,0,1,0,0,1 \ 0,1,1,0,1,1)
    matrix eta = A*b'
    matrix covariance = A*V*A'
    assert abs(eta[1,1]) < 1e-7 & abs(eta[2,1]-ln(3)) < 1e-7
    assert abs(covariance[1,1]-(6/5)*.5/9) < 1e-9
    assert abs(covariance[2,2]-(6/5)*8/81) < 1e-9
    assert abs(covariance[1,2]) < 1e-9
    assert e(df_chi2) == 1 & e(df_m) == 1
    assert abs(e(chi2)-(27/5)*ln(3)^2) < 1e-7
    predict double mean, mu
    assert abs(mean-cond(h==1,1,3)) < 1e-7
}

// Partial projection onto slope-only effects leaves a finite constant direction.
// The cellwise residuals are orthogonal to both one and z=(1,2,3).
// An independent four-column dummy sandwich gives V[x,x]=13/45.
clear
input double(y x z) byte g
1.25 0 1 1
.5 0 2 1
1.25 0 3 1
3.5 1 1 2
2 1 2 2
3.5 1 3 2
end
generate byte f = 1
tempfile slopes
save `slopes'
foreach memory in fast low {
    foreach workers in 1 4 {
        foreach rhs in "x" "x i.f" {
            use `slopes', clear
            quietly feppml y `rhs', absorb(g#c.z) separation(none) ///
                memory(`memory') threads(`workers') mu(mean)
            matrix b = e(b)
            matrix V = e(V)
            assert abs(b[1,1]-ln(3)) < 1e-7
            assert abs(V[1,1]-13/45) < 1e-8
            assert e(df_chi2) == 1 & e(df_m) == 1
            assert abs(e(chi2)-(45/13)*ln(3)^2) < 1e-7
            assert abs(mean-cond(x==0,1,3)) < 1e-7
        }
    }
}

// Prohibited next overrides fail atomically for both absorbed grammars;
// no-FE and ordinary intercept absorption still support minimum overrides.
use `source', clear
generate double z = cos(_n*.11)
foreach effects in g#h g##c.z {
    foreach method in operator fvset {
        quietly regress y x
        matrix old_b = e(b)
        matrix old_V = e(V)
        local rhs ib1.q
        if "`method'" == "fvset" {
            fvset base 1 q
            local rhs i.q
        }
        capture noisily feppml y x `rhs', absorb(`effects') separation(none) ///
            engine(next) mu(should_not_exist)
        assert _rc == 498
        assert "`e(cmd)'" == "regress"
        assert mreldif(e(b),old_b) == 0 & mreldif(e(V),old_V) == 0
        confirm new variable should_not_exist
        fvset clear q
    }
}
quietly feppml y x ib1.q, separation(none)
assert e(converged) == 1
quietly feppml y x ib1.q, absorb(g) separation(none)
assert e(converged) == 1
display "END_FIXTURE|factor_single_level_original_axis|rc=0"
