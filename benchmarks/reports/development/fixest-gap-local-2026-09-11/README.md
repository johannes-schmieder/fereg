# Local investigation of fereg's gap to fixest

The best local prototype makes 50-control absorption roughly three times
faster on the well-connected graphs. It remains 36–38% slower than fixest at
four native threads and about twice as slow at sixteen. One-FE overhead is a
separate problem: the current Stata loading and command boundary costs about
0.45–0.50 seconds at one million observations, in addition to computation.
No SCC job was submitted or resumed for this investigation.

These are development results for an isolated OLS prototype. The production
OLS automatic route has not been replaced. The retained implementation changes
are a small contiguous-slice optimization of the existing Schur kernel,
expanded weighted/thread checks, and optional CMG phase diagnostics. The
experimental OLS adapter and alternative solvers are preserved as patches.

## Repeated command timings

All entries below use 983,040 observations, 50 controls, IID covariance, and
three seeds. Times are medians in seconds. Each command ran in a fresh process
after an untimed small-data warm-up; import, startup, diagnostics and result
extraction were outside the timer. The estimation timer includes covariance.
Command order was rotated. Phase tracing was disabled for these comparisons.

| Design | Native threads | Current fereg | Best OLS prototype | fixest | Prototype / fixest |
|---|---:|---:|---:|---:|---:|
| One FE | 4 | 1.266 | — | 0.803 | — |
| Well-connected, degree 3 | 4 | 7.495 | 2.582 | 1.894 | 1.36 |
| Well-connected, degree 6 | 4 | 6.947 | 2.124 | 1.541 | 1.38 |
| One FE | 16 | 0.999 | — | 0.442 | — |
| Well-connected, degree 3 | 16 | 3.982 | 1.742 | 0.840 | 2.07 |
| Well-connected, degree 6 | 16 | 3.685 | 1.466 | 0.690 | 2.12 |

Both well-connected designs have **two** fixed effects. The degree is the
number of observations per worker; it is not the number of FE dimensions.

Current fereg and the first Schur prototype were measured in the first
alternating comparison block (48 calls). The best prototype and fixest were
measured in a second alternating block (24 calls), using the same three seeds
and exact input values. The one-FE entries come from the first block. The
[primary summary](primary-summary.csv) and [final prototype summary](vector-summary.csv)
include ranges and RSS; [all measurements](measurements.csv) retain every
screen and confirmation run. The first Schur prototype took 2.819/2.288 seconds
at four threads and 1.803/1.499 at sixteen. The final inner-loop change therefore
added a modest gain to the much larger algorithmic improvement.

This machine was an Apple M2 Ultra with 24 cores, including 16 performance
cores, and 192 GiB RAM. Stata 19's internal processor count was capped at eight
by the local license; fereg's Rust team and fixest each received the stated
4/16-thread request. These are not 28-core SCC results. CPU affinity is not
controlled on macOS, and launch load averages are retained. R was 4.6.1;
fixest was source-built 0.14.2. The baseline is commit
`d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd`.

## What is expensive

The degree-three, four-thread profile localizes the initial absorption cost.
In a representative diagnostic run, fereg spent 6.28 seconds in initial
absorption, but only 0.10 in the finite solve and 0.01 in final-residual
absorption. A more detailed CMG profile separated approximately 5.95 seconds
of graph projection, 0.11 of reference norms and 0.22 of fresh certification.
The graph solve itself dominated the projection; building its RHSs and
subtracting fitted FE contributions were much smaller.

Thus the gap is principally an expensive absorption method for this graph
class. Removing scientific checks or changing the finite regression solve
would miss the main opportunity. These profiles are diagnostic calls, separate
from the untraced timings above. [Phase records](phase-timings.json) retain the
warm-up and target records without distributing licensed Stata logs.

The pinned fixest source has a closed-form one-FE path, a specialized two-FE
coefficient-space procedure, Irons–Tuck acceleration, and OpenMP work across
outcome/control columns. Its two-FE iteration avoids repeatedly updating an
observation-length residual vector. The relevant functions are
`demean_single_1`, `compute_fe_coef_2` and `demean_acc_gnl`, followed by the
variable-parallel loop in `src/demeaning.cpp`. The official
[algorithm documentation](https://lrberge.github.io/fixest/reference/demeaning_algo.html)
and [source](https://github.com/lrberge/fixest/blob/master/src/demeaning.cpp)
describe these mechanisms; this investigation used the pinned 0.14.2 archive,
not the floating source branch. Both implementations have compiled numerical
kernels. Switching programming languages is not the finding here.

## The useful prototype

Fereg already contains a two-FE Schur-complement solver in its reusable
categorical absorption code. The diagnostic OLS adapter calls that solver
without the preliminary observation-space MAP sweeps. It analytically
eliminates the larger FE dimension and solves for coefficients on the smaller
FE dimension. With weighted FE totals `D_w`, `D_f` and cross-incidence `B`, the
operator is `D_f - B' D_w^{-1} B`; it is applied without forming that matrix.

For degree three at this sample size, the ordinary CMG graph has 335,872
vertices, whereas the Schur recurrence has 8,192 firm coefficients. The
operator still processes worker memberships, so this is not a claim of
41-fold total work reduction. It does substantially reduce recurrence state
and replaces expensive multigrid-preconditioned iterations with cheaper
matrix-free iterations. The first prototype used 16 iterations in the profiled
degree-three case, versus CMG's 19: the gain is mostly cheaper iterations.

The final kernel change takes checked contiguous RHS slices before the
innermost accumulation loops. This avoids repeated indexed accesses and leaves
each column's arithmetic and observation order unchanged. All twelve final
prototype confirmations produced **identical exported full coefficients and
covariance matrices** to the first Schur prototype. Weighted kernel tests
separately compare actual floating-point bits across thread counts. Both
[focused kernel tests](kernel-tests.json) passed, including 52 active columns
and a 51-column fit in a 64-column workspace. The
[normal universal macOS build](retained-build.json) also passed its binary
audit and actual-plugin ABI/self-test on the arm64 host.

The prototype uses the same public `1e-8` tolerance, solves its internal Schur
problem at `1e-10`, and retains fresh FE, finite-score, rank, inverse/bread and
covariance certification. Fixest retains its native defaults; its stopping
criterion is not numerically interchangeable with fereg's. Final outputs were
checked directly, rather than declaring two tolerance option values equivalent.

## One-FE overhead

| Native threads | Total fereg | Input loading | Native computation | Outside the native call, excluding preflight |
|---|---:|---:|---:|---:|
| 4 | 1.266 | 0.291 | 0.766 | 0.205 |
| 16 | 0.999 | 0.294 | 0.543 | 0.157 |

These are independently computed medians; components need not sum exactly to
the median total. Preflight itself was approximately 0.0004 seconds. The final
column includes Stata-side work and publication after the native runtime's
pre-store timestamp; it is not solely syntax parsing.

At sixteen threads, approximately 0.45 seconds of current loading/boundary
cost already equals fixest's entire 0.44-second call. This is a limit of the
current implementation, not a universal lower bound for Stata. It makes parity
unlikely from absorption optimization alone. At four threads, raw RHS
construction, moments, norms, one-pass absorption and finite regression also
account for several substantial memory passes.

The narrow-control checks reinforce this interpretation. With one million
rows, one control took 0.173 seconds in fereg versus 0.076 in fixest; ten
controls took 0.346 versus 0.179. At 7,680 rows, one-control calls were
0.006/0.009 seconds and ten-control calls 0.011/0.010. Those tiny-call figures
are single diagnostic measurements near timer resolution. They do not suggest
a large universal fixed ado overhead.

Three possible shortcuts were investigated and rejected: column-wise SPI
reads increased the one-FE call to 1.90 seconds; parallel column-norm blocks
did not improve it; removing the sort-preservation guard saved only a small
amount in the simple fixture and would discard a useful public guarantee.
The row-wise bulk loader and the guard were retained. A useful next one-FE
step is to investigate duplicate full-data passes and ownership transfers,
including reuse of already-certified raw moments, while maintaining sample
and missing-value semantics.

## Tradeoffs and alternative experiments

The Schur adapter is not ready to replace all two-FE routes. On the independent
weakly connected holdout (61,440 rows, 50 controls, four threads), current
fereg took 0.362 seconds, fixest 0.399, and the prototype 0.763. This is one
measurement, not a claim of a statistically established fereg win. It is a
clear warning against unconditional Schur routing: the current solver is useful
on difficult connectivity. On the separate panel holdout, the prototype was
1.239 seconds versus current fereg's 1.320 and fixest's 1.054; another panel
seed in the initial screen favored current fereg.

Other screens clarified the alternatives:

- Explicit observation-space MAP and Irons–Tuck were faster than current CMG
  on some well-connected cases, but remained materially behind fixest.
- Forming the smaller Schur graph explicitly and applying CMG helped the panel
  with repeated links, but added graph-construction/fill work and lost to the
  matrix-free prototype on the regular degree-three/six graphs.
- Independent narrow-column Schur tasks, including tiled copies, did not beat
  the full-width matrix-free implementation. Copying fixest's parallel work
  distribution alone is insufficient for fereg's current data structures.
- The existing `poolsize()` is a practical tuning opportunity. With 52 actual
  RHSs (outcome, controls and structural intercept), default batches of ten
  leave partial waves at four threads and cannot use sixteen concurrent RHS
  workers. `poolsize(8)` at four threads gave 6.420/6.021 seconds for degree
  three/six; `poolsize(16)` at sixteen threads gave 3.513/3.264. These single
  diagnostic calls saved about 10–16% against the default comparison and were
  bit-identical in full exported coefficients and covariance. They do not
  explain or close the larger algorithmic gap.

Memory remains a relative strength in these local measurements. At four
threads, the best prototype's median absolute process-tree RSS was 1.73 GiB
for degree three and 1.52 for degree six, versus fixest's 1.87/1.86 GiB. The
one-FE baseline was 1.39 versus fixest's 1.80 GiB. The prototype adds workspace
relative to current fereg in some cases; observed RSS is not a substitute for
integrating that workspace into the production memory planner.

## Evidence and next decision

The [scientific checks](science.json) cover 93 fereg-to-fixest comparisons and
five independent explicit-dummy oracles. There were 143 completed estimator
invocations across screens, confirmation blocks and validation; every worker
completed successfully. The checks retain exact samples, coefficient statuses,
rank and DoF, native certificates, full native coefficient/covariance parity
including the reported constant, and reference slope estimates/covariance/RSS.
The explicit-dummy oracle uses SciPy's separately implemented sparse solve and
finite regression on five 7,680-row graph designs. This is local development
evidence, not release or cross-platform qualification.

The next substantial improvement should be a guarded OLS Schur route for the
well-connected, control-heavy region, with the existing CMG/LSMR routes retained
where they are effective. Its admission needs explicit workspace accounting,
precise solver/iteration reporting, a bounded continuation strategy, and
holdouts that separate connectivity from simple FE counts. A graph-aware guard
or cheap coefficient-space convergence probe is more defensible than selecting
a method by these benchmark names. The current accepted policy is in
[ADR 0021](../../../../docs/adr/0021-akm-auto-routing-recalibration.md) and its
terminal guards; the diagnostic adapter does not silently amend it.

Getting all the way to fixest will also require reducing the serial/data-movement
portion, especially at higher thread counts. The measured result supports
substantial improvement and a clear next implementation target; it does not
support a claim that fereg now beats fixest on these cases.

[Source/artifact identities](identities.json), [prototype patches](patches/),
[numeric evidence archive](evidence.zip), and the
[local reproduction harness](../../../local_fixest_gap/README.md) preserve the
work. Raw licensed logs, datasets and binaries are excluded from this report.
