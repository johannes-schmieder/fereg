# Start final residual work before constructing the raw residual

The adopted schedule starts the existing final residual projection before the
independent raw-residual bank is constructed. That bank belongs to reported
bread and is now constructed inside the main bread job. Its allocation,
values, arithmetic and error position remain unchanged; projection errors stay
deferred until raw-residual/bread and within-TSS checks complete. The existing
narrow one-FE, unit-weight, >=16,384-row, >=8-worker guard bounds this overlap.
No owner, thread or numerical shortcut is added. Allocation-phase probes retain
the original sequential path.

Three-pair screening gives predecessor/candidate/fixest medians of 138/134/150 ms
for ten regressors and 398/393/488 ms for fifty. A fresh five-pair confirmation
gives 137/134/151 ms and 395/391/454 ms. Narrow bread/completion timing falls
from 13.23 to 11.39 ms in the screen and 13.00 to 11.29 ms in confirmation.
The wide case is outside the scheduling guard; its small native timing changes
are not attributed to this optimization.

All eleven protected cases pass: maximum median paired candidate/predecessor
ratio 1.010076 and geometric mean 0.991875. There are 124 accepted development
calls and 108 byte-identical native coefficient, covariance, status, DoF,
certificate and fit exports. Both artifact ABI checks, six licensed fixtures
and four focused native test executions pass. Two tests and three fixtures
belong to the separately rejected parallel-mean-scale experiment. Its patch
and measurements remain here: the parallel finite maximum preserves mean bits
but adds no reliable command benefit (134/134 ms narrow, 391/395 ms wide).
The retained raw-residual schedule passes complete-fit comparisons and the
45-case residual perturbation/error-order test.

Two preselected five-pair final blocks add 40 accepted calls. In block A,
candidate/fixest medians are 132/146 ms (K=10) and 390/450 ms (K=50); median
paired ratios are 0.897959 and 0.862921. Block B gives 134/140 ms and 395/479 ms,
with ratios 0.963504 and 0.821206. Fereg wins nine of ten narrow pairs and all
ten wide pairs. The strict repeated 5% target is **not met** because block B's
narrow median paired ratio exceeds 0.95. Preserve both blocks and the original
plan. Further work should target the remaining serial completion critical path.

All primary cases use 983,040 rows, one ordinary intercept FE, numeric regressors,
unit weights, IID inference and 16 native threads on the local M2 Ultra.
The reference is the same pinned fixest 0.14.2 archive as preceding reports.
This is development evidence, not release or cross-platform qualification.
No installed package, remote state or reference pin was changed.
The candidate plugin SHA-256 is
`c2a40fec9e01ddccffa72a0e2e65a171d407b0c9ea68d300ccfb0551c2bdb8ac`.

The completion diagnostic adds eight accepted calls with an instrumented copy
and its own passing ABI check; the source-matched production reference has the
licensed fixtures above. Narrow job medians are raw residual construction
1.61 ms, residual projection 6.90 ms, score checking 4.44 ms and independent
RSS 3.84 ms. Projection precedes score/RSS; score and RSS overlap, and raw
construction belongs to the concurrent bread job. Do not add all four times.
The width guard excludes these timers at K=50. Instrumentation preserves exact
native science. The next measured boundary is the two serial finite-value scans
and unit norm inside residual projection.
