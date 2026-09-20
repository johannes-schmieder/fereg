# Compact weighted Schur gathers in existing scratch

Local development measurements on 2026-09-20 on M2 Ultra, Stata/MP 19.0
(8 licensed processors) and pinned stock fixest, with 16 requested native/R
threads unless specified. The embedded build date is September 19. These
focused measurements do not close the broad milestone or qualify a release.

Candidate `weightedboth` is base `f44fabb9` plus the recorded core overlay;
all 516 captured source files match. Plugin SHA-256:
`e2325d207fa4a7a5afe23c9cecf0c4bcc73c383294586f7e31f3fca317b023f3`.
The parent is the accepted `unitboth` artifact. Identities, shared input hashes,
all 100 successful application calls, pair order, fixture sentinels and source
patch are retained here. No installed package or remote state changed.

The existing weighted right-edge cache is compacted in place and transposed
into disjoint parts of the same code and weight banks. Both operator gathers
then use identical full-precision normalized edge sums. With E unique edges,
code storage must fit max(N,2E)+(L+1)+(R+1) words and weight storage must fit
max(N,2E); otherwise the existing one-sided cache remains valid. Offsets start
past the old N slots and both compact edge ranges. There is no new heap owner,
allowance, weight approximation or weaker certificate. Existing capped-OLS,
worker, repeated-edge, restoration and fallback rules remain. Shared/PPML
calls opt out. [ADR 0083](../../../../docs/adr/0083-compact-weighted-ols-edge-scratch.md)
records the decision.

## Timing

Milliseconds below are separate medians of three paired runs. Every paired
ratio, minimum/maximum and individual measurement is preserved in the receipts.

| AW cell | Parent | Candidate | Stock fixest |
|---|---:|---:|---:|
| Shuffled K=6 IID | 336 | 316 | 298 |
| Shuffled K=6 full outputs | 486 | 466 | 545 |
| Ordered K=6 IID | 290 | 285 | 480 |
| Ordered K=6 full outputs | 436 | 429 | 707 |
| Ordered K=50 IID | 770 | 747 | 1957 |
| Ordered K=50 full outputs | 955 | 928 | 2150 |

The shuffled IID median paired native ratio is 0.94048. It remains slower than
fixest: this checkpoint narrows, but does not close, that gap. The separate
profile records initial operator 65.27 -> 38.43 ms, initialization
8.53 -> 10.64 ms, and initial absorption 149.26 -> 132.35 ms. Both use 35
iterations. Weighted IID setup/statistics remains about 21 ms after the fit.

PW/FW K=6 robust, crossed cluster, two-cluster/saved and other saved/full-output
checks all pass. Single-pair concentrated-weight outputs are 1.6% slower and
the unchanged four-thread path 3.0% slower. No cell median paired native ratio
exceeds 1.05 (maximum 1.02969). Three-pair shuffled unit K=5 is 261 ms in both
arms; degree-six AW is 82 -> 80 ms. The one-FE, low-memory and weighted-bridge
controls pass. Timing differences on ineligible controls are not attributed
to the compact cache. These limited repetitions are diagnostic; the full
predeclared control and holdout campaigns remain open.

## Science and memory

All 41 native paired numerical comparisons pass, with exact status/DoF and
sample identity. Twelve preserve all six scientific files bitwise. The
registered finite-coefficient comparison has zero error; maximum SE error is
1.11e-16 and covariance error 6.36e-22. All sixteen saved-output comparisons
pass numerically, with two bitwise exact. All sixteen independent weighted
normal-equation, component-normalization, RSS and reconstruction audits pass.
Nine default-fixest output disagreements remain explicit; none of the
coefficient comparisons failed. Frequency controls use the native parent;
ordinary fixest weights are not substituted for frequency semantics.

No native allocation peak increases in any of 41 pairs. Thirteen focused
Schur tests pass, including independently assembled weighted adjacency sums,
left/right edge ordering, exact-capacity rejection, unchanged one-sided
fallback, arbitrary fractional weights, common scales from 1e-100 to 1e100,
connected/disconnected projections against MAP, 8/16 workers and forced
capped continuation. Existing absolute oracle bounds are unchanged. The
actual-plugin ABI/panic test and three Stata fixtures pass with explicit
fixture and terminal sentinels; the weighted fixture independently compares
AW/FW/PW inference and saved-output reconstruction to `areg` plus explicit
second-effect dummies.

Next profile the remaining weighted statistics passes, then reduce duplicate
probe/initialization and restoration work. The 111 native controls, sixteen
fresh-seed holdouts and two final five-pair blocks remain necessary before
claiming the broad development milestone.
