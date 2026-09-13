# P5 bounded parallel FE certificate: unadopted native revision

The [182-input source snapshot](receipt.json) changes the centered initial FE
certificate and its caller from the [parallel Gram stage](../parallel-gram/README.md).
Each fixed 1024-group partial reuses the existing stable group-mean arithmetic.
Absolute/relative maxima and acceptance remain exact; only the diagnostic
relative two-norm changes summation order across fixed chunks. The requested
team runs the work without SPI. One-worker/small inputs remain serial.

Three doubles per group chunk are fallibly reserved within the remaining
frame component budget and actual capacity is charged cumulatively. If the
partial bank does not fit, the original serial certificate runs without it.
The buffer is 4608 bytes on the million-row target and 1152 on the smaller
ones. It is released before finite fitting and does not change observed peaks.
This is component admission, not proof of the complete FE/inference envelope.

All 52 distinct final centered/robust/clustered tests and non-test core checking
pass. The new test covers 4099 groups (including the final partial), 1/65 RHSs,
unit/unequal weights, 1/4/16 workers, corrupted means, exact maxima/acceptance,
repeat determinism, zero/119/120-byte budgets, injected failure and oversized
capacity rejection. At 119 bytes the serial certificate needs no allocation;
120 bytes admits the bounded parallel bank. The first test failed to compile
because its fixture borrowed a temporary effect; a local binding fixes that
lifetime, with the failed log preserved. No scientific assertion was relaxed.

Both plugin slices visibly rebuild. Universal static audit and actual arm64
ABI pass; candidate plugin is
`41555c29464bbfc2234414f80111388c2c48d78cd0b1db0670f8c23a2c94a18d`.
Accepted baseline remains `c64ead07`; installed dist is unchanged.

| Original first screen | Accepted baseline → candidate seconds | Change | Native peak reduction |
|---|---:|---:|---:|
| Unit IID, N983040 / K50 / 16 workers | 0.573 → 0.631 | +10.1% | 42.7% |
| AW robust, missing, N245760 / K50 / 4 | 0.423 → 0.433 | +2.4% | 42.3% |
| PW two-way cluster, missing, N245760 / K50 / 16 | 0.322 → 0.342 | +6.2% | 40.4% |

AW/PW meet the prospective first-screen memory/time condition. IID remains
outside its <=10% slowdown condition; this is not an adoption. The prior
unadopted serial-FE candidate took 1.037/0.495/0.419 seconds on these recipes
and seed; these separate screens are not repeated candidate-to-candidate pairs.
All native peaks remain 530340930/134605158/142507032 bytes and fit unchanged
public models. Reported scratch is zero. Full science checks and independent
certificates pass; sample/status/rank/DoF and FE/cluster structure remain exact.

Two separate profile pairs use seed67280421 and are not pooled. IID is
0.569→0.640 seconds, with fit 0.15999→0.22933; AW is 0.420→0.433, with fit
0.10334→0.15683. Earlier separate profiles recorded centered fit at 0.63615 for IID in the
native-mean-bank stage and 0.22064 for AW in the parallel-Gram stage. AW score design stays 0.03415 seconds
against accepted baseline 0.08984, while covariance is 0.10249 versus 0.08776.
Thus substantial initial-fit overhead remains despite both certificate gains.
Setup/solve phase meanings overlap other work; do not add their independent
medians or treat these single profiles as isolated kernel benchmarks.

Primary sampled RSS falls, but the separate AW profile's RSS rises
0.3819→0.3982 GiB. It is diagnostic process sampling, not a native capacity
bound. Numeric results, solver/time/memory exports and sanitized traces are
archived with checksums. Raw licensed logs remain outside Git.

Next resolve the near-half mean-bank/QR boundary and full overlapping memory
admission, then remaining IID overhead and protected controls/repeats. No
additional primary seeds, protected command campaign or PPML centered evaluation
has run. The direct/SD saved-output selection remains included but unadopted.
Every P5 checkbox remains open; this evidence does not approve the route.
