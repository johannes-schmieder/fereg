# Generic MAP initialization: remove an unused certificate

Local Mac development milestone, 2026-09-22. Baseline `projrow1` matches source
`7ef711a8`. Candidate `mapinit1` is that source plus the archived `map.rs`
overlay; all 529 captured build inputs match the resulting computational
source. Candidate plugin SHA-256:
`97271f6c7980a31b3601bf32a419469c10f3c721e51a286257d07854af0a63d8`.
Baseline plugin:
`4bec0e08a01eb8f642c87f9398186d1c3345ce1b9ba3b090b78c6c4b9bc5c522`.

## Finding and change

The previous milestone left generic owned-FE certification as a dominant
cost. Lifecycle inspection found that one-shot MAP calculates an initial full
QR/span certificate, performs a sweep unconditionally, and overwrites that
certificate at the first scheduled check. It neither controls stopping nor
supplies a returned result. The automatic router's separate initial certificate
does control contraction and remains necessary.

[ADR 0115](../../../../docs/adr/0115-map-initial-certificate-lifetime.md) removes
only the unused calculation. Configuration, shape, weights and initial RHS
norm checks remain. Before modifying the matrix, MAP explicitly checks all
weighted basis entries for the same nonfinite-value error. The empty-RHS
exception and no-FE path retain their previous behavior. All scheduled and
final-iteration certificates, returned certificate values, iteration counts,
routing, tolerance, rank, DoF and publication rules remain unchanged.

This serves generic MAP, its automatic pilot and MAP fallback callers. The
changing-weight reusable MAP implementation is separate and unchanged; there
is no persistent OLS or PPML cache, new buffer, worker team, API or admission
allowance. Fresh residual, score and final-Hessian certificates remain binding.

Separate native diagnostics at N61440/K10 show initial-batch certificate calls
fall from 10 to 9 on both designs, and final-residual calls from 3 to 2. Initial
certificate time falls 175→157 and 204→182 ms; final-residual certification
falls 25→17 and 24→16 ms. Projection counts remain 36/55 initially and 3/5
finally. These instrumented timings explain the mechanism and are excluded
from the headline table.

## Pinned comparator diagnostic

An isolated extraction of the pinned fixest 0.14.2 archive uses the existing
timer-only `benchmarks/one_fe_profile/instrument.py`. Its two altered sources,
original source hashes, instrument hash, compiler log and separate library
identity are retained. No numerical statement or stock library is changed.
Requested/effective thread counts are checked.

| Existing diagnostic input | FE setup | Workspace | Iteration work | Subtract/export | Iterations per RHS |
| --- | ---: | ---: | ---: | ---: | --- |
| Two slope partitions, N61440 K10 T4 | 1.5 ms | .4 ms | 37.4 ms | 1.6 ms | 6 |
| Unbalanced three FE, N61440 K10 T4 | 2.0 ms | .4 ms | 55.6 ms | 1.5 ms | 5 |
| Two slope partitions, N76800 K33 T1 | 1.3 ms | 1.2 ms | 387.0 ms | 23.8 ms | 5–6 |

All three diagnostic b/V exports, sample hashes, N, DoF and RSS match the
previously captured stock results exactly. The diagnostics use those archived
input designs and do not supply headline comparisons for this candidate.
Fixest precomputes small slope systems, then iterates over FE coefficients,
with RHS work distributed over its requested team. Its stopping checks are
not substitutes for fereg's independent rank-aware certificates. The remaining
speed gap therefore cannot be attributed solely to small-system factorization.

The first diagnostic build lacked the documented SDK C++ header path and
failed before producing a library. The corrected build passes. A first
standalone diagnostic worker retained its supervised readiness wait and
inherited a one-thread OpenMP setting; that exact process was stopped before
the target fit. Its request, warning log and interruption receipt remain.
The corrected run removes the standalone wait, uses the existing per-cell
thread environment and verifies effective threads. Neither failed setup enters
any timing comparison.

## Alternating performance and losses

Three repetitions rotate baseline/candidate/stock order. All compilation,
profiling, independent oracles and archiving occur outside headline blocks.
Dataset import and input hashing precede timing; native SPI loading, internal
preparation, estimation and public result formation remain inside the command.
No additional saved outputs are requested here; licensed saved-slope fixtures
protect reconstruction separately. Low memory is native's requested storage
mode, not an equal-budget stock comparison. Times are command medians in ms;
ratios are paired candidate/baseline minimum, median and maximum.

| Case (N, K, threads) | Baseline | Candidate | Stock | Paired ratio min / median / max |
| --- | ---: | ---: | ---: | --- |
| Two slope partitions (61440,10,4) | 333 | 310 | 57 | .914 / .937 / .949 |
| Unbalanced three FE, two slope partitions (61440,10,4) | 386 | 361 | 75 | .870 / .933 / .955 |
| Fresh wide two-partition slopes (76800,28,1) | 780 | 725 | 558 | .921 / .928 / .937 |
| Fresh unbalanced three FE (76800,18,8) | 665 | 612 | 113 | .897 / .917 / .920 |
| Fresh low-memory slopes (30720,12,1) | 381 | 360 | 90 | .928 / .940 / .963 |
| Weak native slopes (19200,7,4) | 86 | 80 | unranked | .920 / .930 / .953 |
| AW robust slope control (30720,10,4) | 304 | 287 | — | .937 / .944 / .957 |
| PW clustered slope control (30720,10,4) | 303 | 286 | — | .941 / .944 / .947 |
| Categorical one-FE control (61440,10,4) | 20 | 20 | — | .950 / 1.000 / 1.111 |
| Categorical two-FE control (61440,10,4) | 79 | 78 | — | .987 / .987 / 1.000 |
| Public categorical PPML control (61440,6,4) | 236 | 230 | — | .958 / .987 / 1.000 |

Eight slope rows improve 5.6–8.3% by paired medians. All five comparable stock
rows still favor fixest. The weighted controls preserve deliberately absorbed
x1/x2 finite-column omissions; ordinary fixest weights are not treated as a
Stata weight-inference oracle. Categorical and PPML changes are small and do
not establish improvements to their separate algorithms. A one-FE pair loses
2 ms (11.1% at this small scale); it remains in the distribution. No tuning,
guard changes or favorable repetition selection follow these results.

## Validation, memory and limits

All 81 headline application calls succeed, without timeout, detected overlap
or monitor error. All 33 native pairs and 44 repeat checks pass the unchanged
scientific envelopes. All 33 pairs have byte-identical scientific exports,
including covariance, status, DoF and certificates. A separate audit verifies
all non-clock solver fields exactly, including routes, preconditioners,
iterations, fallback and contraction. All 15 direct stock headline comparisons pass.

Three bounded independent pilots cover strong two-partition slopes,
unbalanced three-FE slopes and the positive-DoF weak barbell. Baseline,
candidate and stock pass b/V/sample/RSS/rank checks on these inputs. The earlier
weak-stock covariance precision failure is not erased by a fresh passing seed;
the larger weak timing remains native-only. The earlier three-partition
exact-rank/conservative-DoF distinction and disconnected stock discrepancies
remain separate. No stopped weak_d3 or empirical/reference campaign is resumed.

Additional checks pass:

- 31 distinct Rust tests: three MAP tests, twelve automatic routing/budget
  tests, eight independent FE-certificate oracles and eight slope oracles.
- The MAP regression compares the previous implementation across zero/one/seven
  RHSs, unit/unequal weights, symmetric/asymmetric sweeps, iteration budgets
  1/3/8/100 and check intervals 1/4/99. Returned residual bits, certificates,
  iteration counts and stop reasons match. Initially orthogonal inputs still
  wait for their first scheduled check. Weighted-basis overflow returns the
  same error before any input mutation.
- Actual-plugin ABI/panic containment and four licensed fixtures: multiple
  slope-only OLS, intercept-plus-slope OLS, public PPML saved slopes and OLS
  eclass transaction atomicity. Return codes and both fixture/terminal sentinels
  are explicitly verified.

All 33 paired tracked native heap peaks match. No new buffer or cache is
introduced and admission remains conservatively unchanged. This does not
establish a tighter whole-action memory bound. The static allocation audit
continues to fail documented source drift and is not regenerated. Weighted
constrained fits, nonretaining CG span instability, factor-base provenance and
SCC/Linux confirmation remain open. Frozen 77/16/111 definitions and reserved
seeds 202609211/212 are untouched. New paired/pilot/diagnostic seeds are
202609265–270; the comparator diagnostic reuses identified earlier inputs.

Close the initialization experiment. The next substantial opportunity is the
remaining owned-FE certificate geometry and execution schedule. A scoped cache
would need immutable design/current-weight ownership, honest construction and
solve peaks, and a fallback that does not displace affordable existing routes.
Measure that independently; retain fresh residual/span checks and unchanged
checkpoint frequency. Parallel work must also preserve deterministic reduction
and routing behavior. A cache is not implemented or accepted here. This is a
local development checkpoint, not broad completion, qualification or release.

## Immutable evidence

The receipt archive preserves declarations, exact native identities, captured
controllers and input hashes, timing distributions and losses, independent
pilots, native/stock phase diagnostics, original and instrumented comparator
sources, tests and sanitized licensed-fixture protocols. Data, binaries and
raw licensed Stata logs are excluded. The candidate's 529 captured inputs and
archive entries are hash-verified.

[Receipt archive](receipts.zip) and [checksum](receipts.sha256): 2,996 verified
entries plus the hash manifest, 4,850,599 compressed bytes. SHA-256:
`8c110b6a45f0135f6bd58c28d89eea30dd0a0640a4569b10046ca8248382c1df`.
