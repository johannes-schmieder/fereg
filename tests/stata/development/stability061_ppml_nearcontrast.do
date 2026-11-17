version 18.0
clear all
set more off

* These are exact binary contrasts, not tiny numerical perturbations of an
* intended duplicate column. For x2=x1+2^-k*zero, 2^k*(x1-x2) equals -1
* exactly on every y=0 row and zero on every positive row. Canonical numeric
* rank may decline this difficult basis, but cannot certify no separation
* merely because its two individual raw score moments are small.
set obs 48
generate byte g = 1 + floor((_n - 1) / 12)
generate byte zero = mod(_n, 2)
generate double y = cond(zero, 0, 1 + mod(_n, 3))
generate double x1 = mod(_n, 3) - 1
generate double x2 = x1

foreach k in 40 44 48 50 {
    quietly replace x2 = x1 + 2^(-`k') * zero
    assert 2^`k' * (x1 - x2) == -zero
    foreach shape in explicit absorbed split {
      local rhs "x1 x2"
      local absvars "g"
      if "`shape'" == "absorbed" {
          local rhs ""
          local absvars "g##c.(x1 x2)"
      }
      if "`shape'" == "split" {
          local rhs ""
          local absvars "g g#c.x1 g#c.x2"
      }
      foreach methods in default ir {
        local options ""
        if "`methods'" == "ir" local options "separation(ir)"
        quietly feppml y x1 if !zero, absorb(g) threads(1)
        matrix previous_b = e(b)
        matrix previous_V = e(V)
        generate byte previous_sample = e(sample)
        capture noisily feppml y `rhs', absorb(`absvars') `options' threads(1) eta(proposed_eta)
        local result = _rc
        if `result' == 0 {
            assert e(N) == 24
            assert e(sample) == !zero
            assert missing(proposed_eta) if zero
            drop proposed_eta
        }
        else {
            assert inlist(`result', 430, 459)
            assert e(sample) == previous_sample
            assert mreldif(e(b), previous_b) == 0
            assert mreldif(e(V), previous_V) == 0
            confirm new variable proposed_eta
        }
        drop previous_sample
      }
    }
}

* Joint omitted contrasts: each small residual is dominated by the same
* balanced positive-row direction. Their much smaller exact difference is
* nevertheless a separator. Per-column normalized moments alone cannot
* establish orthogonality to the joint omitted span.
clear
set obs 64
generate byte g = 1
generate byte zero = mod(_n, 2)
generate byte pattern = mod(floor((_n - 1) / 2), 4)
generate double y = 1 - zero
generate double x1 = cond(zero, 0, cond(mod(pattern, 2), 1, -1))
generate double u = cond(zero, 0, cond(pattern < 2, 1, -1))
generate double x2 = x1
generate double x3 = x1
foreach k in 40 44 {
    quietly replace x2 = x1 + 2^(-`k')*u
    quietly replace x3 = x2 + 2^-100*zero
    assert x3 - x2 == 2^-100*zero
    quietly feppml y x1 if !zero, absorb(g) threads(1)
    matrix previous_b = e(b)
    matrix previous_V = e(V)
    generate byte previous_sample = e(sample)
    capture noisily feppml y x1 x2 x3, absorb(g) separation(ir) threads(1) eta(proposed_eta)
    local result = _rc
    if `result' == 0 {
        assert e(N) == 32
        assert e(sample) == !zero
        assert missing(proposed_eta) if zero
        drop proposed_eta
    }
    else {
        assert inlist(`result', 430, 459)
        assert e(sample) == previous_sample
        assert mreldif(e(b), previous_b) == 0
        assert mreldif(e(V), previous_V) == 0
        confirm new variable proposed_eta
    }
    drop previous_sample
}

display as result "END_FIXTURE|stability061_ppml_nearcontrast|rc=0"
