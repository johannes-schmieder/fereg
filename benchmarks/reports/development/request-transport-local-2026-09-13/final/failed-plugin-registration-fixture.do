version 18.0
clear all
set more off
set obs 4096
generate int g = 1 + mod(_n-1,16)
generate int cl = 1 + mod(floor((_n-1)/19),97)
local xs
forvalues j=1/60 {
    local name "long_transport_control_number`j'"
    generate double `name' = sin(2*c(pi)*`j'*(_n-1)/_N) + cos(2*c(pi)*(`j'+97)*(_n-1)/_N)
    local xs `xs' `name'
}
generate double w = 0.2 + mod(_n,13)/3
generate double fw = 1 + mod(_n,7)
generate double y = 1 + mod(floor(100*exp(0.2*long_transport_control_number1 - 0.1*long_transport_control_number2 + 0.03*g)),19)

// Long source and term names exercise metadata transport even at modest rank.
quietly regress y `xs' [aw=w], vce(robust)
matrix oracle_b = e(b)
matrix oracle_V = e(V)
fereg y `xs' [aw=w], vce(robust) threads(4)
mata: assert(max(abs(st_matrix("e(b)")-st_matrix("oracle_b")) :/ (1e-10 :+ 1e-7:*abs(st_matrix("oracle_b")))) <= 1)
mata: assert(max(abs(st_matrix("e(V)")-st_matrix("oracle_V")) :/ (1e-10 :+ 1e-6:*abs(st_matrix("oracle_V")))) <= 1)
assert e(sample) == 1
assert e(fereg_certificate)[32,2] == 1

mata:
void transport_dummy_sandwich(string scalar xs, string scalar kind)
{
    g = st_data(.,"g")
    G = J(rows(g),16,0)
    for (j=1; j<=16; j++) G[.,j] = (g :== j)
    X = (st_data(.,tokens(xs)),G)
    y = st_data(.,"y")
    mu = st_data(.,"mu_dummy")
    w = st_data(.,"raw_w")
    Hinv = invsym(quadcross(X,w:*mu,X))
    if (kind == "pw") {
        cl = st_data(.,"cl")
        sums = J(97,cols(X),0)
        for (i=1; i<=rows(X); i++) sums[cl[i],.] = sums[cl[i],.] + w[i]*(y[i]-mu[i])*X[i,.]
        meat = quadcross(sums,sums)
    }
    else meat = quadcross(X,w:*(y-mu):^2,X)
    V = Hinv*meat*Hinv*st_numscalar("correction")
    st_matrix("V_dummy",V[1..60,1..60])
}
end

generate double raw_w = fw
foreach kind in fw pw {
    local weight "[fw=fw]"
    local covariance "robust"
    replace raw_w = fw
    if "`kind'" == "pw" {
        local weight "[pw=w]"
        local covariance "cluster cl"
        replace raw_w = w
    }
    feppml y `xs' `weight', absorb(g) vce(`covariance') separation(none) keepsingletons threads(4) mu(mu_native)
    matrix b_native = e(b)
    matrix V_native = e(V)
    scalar correction = e(feppml_inference)[3,1]
    assert e(feppml_certificate)[16,1] == 1
    assert e(converged) == 1
    assert e(sample) == 1
    quietly poisson y `xs' ibn.g `weight', noconstant nolog
    forvalues j=1/60 {
        assert abs(_b[long_transport_control_number`j'] - b_native[1,`j']) <= 1e-7
    }
    predict double mu_dummy, n
    assert abs(mu_native-mu_dummy) <= 1e-7*(1+abs(mu_dummy))
    mata: transport_dummy_sandwich("`xs'","`kind'")
    matrix V_slopes = V_native[1..60,1..60]
    mata: assert(max(abs(st_matrix("V_slopes")-st_matrix("V_dummy")) :/ (1e-10 :+ 1e-6:*abs(st_matrix("V_dummy")))) <= 1)
    drop mu_native mu_dummy
}

quietly regress y `xs'
matrix before_b = e(b)
matrix before_V = e(V)
generate byte before_sample = e(sample)
// Read past the declaration by one content byte before rejecting truncation.
local fereg_request_bundle "1:3:a=btrailing-data"
foreach action in estimate-ols estimate-ppml {
    capture noisily plugin call __fereg_estimate_plugin, "`action'" "bundle_local=7"
    assert _rc == 198
    capture noisily plugin call __fereg_estimate_plugin, "`action'" "bundle_local=16777217"
    assert _rc == 909
    assert "`e(cmd)'" == "regress"
    assert e(sample) == before_sample
    assert mreldif(e(b),before_b) == 0
    assert mreldif(e(V),before_V) == 0
}
replace y = -1 in 1
capture noisily feppml y `xs', absorb(g) threads(4) mu(failed_mu)
assert _rc == 459
assert "`e(cmd)'" == "regress"
assert e(sample) == before_sample
assert mreldif(e(b),before_b) == 0
assert mreldif(e(V),before_V) == 0
confirm new variable failed_mu

display as result "END_FIXTURE|local_request_transport_oracle|rc=0"
